// Modified by JaiMesh contributors in 2026 from the OpenAI Codex source.
use super::*;
use crate::session::tests::make_session_and_context_with_rx;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::approvals::NetworkPolicyAmendment;
use pretty_assertions::assert_eq;

#[test]
fn approval_resolution_rejects_denied_network_policy_amendment() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment: NetworkPolicyAmendment {
                host: "denied.example.com".to_string(),
                action: NetworkPolicyRuleAction::Deny,
            },
        },
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Rejected(rejection)) if rejection == "rejected by user"
    ));
}

#[test]
fn approval_resolution_rejects_mcp_policy_amendment() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::ApprovedMcpPolicyAmendment,
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Rejected(rejection)) if rejection == "Error while requesting approval"
    ));
}

#[test]
fn approval_resolution_aborts_turn_when_approval_is_aborted() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::Abort,
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Codex(error))
            if matches!(
                error.details(),
                codex_protocol::error::CodexErrorDetails::TurnAborted
            )
    ));
}

#[test]
fn approval_resolution_uses_acting_model_timeout_instructions() {
    let mut model = model_info_from_slug("acting-model");
    for timeout_instructions in ["Catalog timeout instructions.", ""] {
        model.model_messages = Some(
            serde_json::from_value(serde_json::json!({
                "auto_review": {
                    "timeout_instructions": timeout_instructions,
                },
            }))
            .expect("model messages should deserialize"),
        );
        let resolution = ApprovalResolution {
            decision: ReviewDecision::TimedOut,
            source: ApprovalResolutionSource::Guardian,
        };

        assert!(matches!(
            resolution.into_tool_result(&model),
            Err(ToolError::Rejected(rejection)) if rejection == timeout_instructions
        ));
    }
}

#[cfg(unix)]
#[test_case::test_case(ApprovalsReviewer::User, codex_extension_api::ApprovalDecision::AskUser; "manual prompt")]
#[test_case::test_case(ApprovalsReviewer::AutoReview, codex_extension_api::ApprovalDecision::Allow; "cached allow")]
#[tokio::test]
async fn non_utf8_cwd_preserves_approval_routing(
    reviewer: ApprovalsReviewer,
    decision: codex_extension_api::ApprovalDecision,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use codex_extension_api::ApprovalDecision;
    use std::os::unix::ffi::OsStringExt;

    struct Contributor {
        cwd: codex_utils_path_uri::LegacyAppPathString,
        decision: ApprovalDecision,
    }

    impl codex_extension_api::ApprovalReviewContributor for Contributor {
        fn decide<'a>(
            &'a self,
            input: &'a codex_extension_api::ApprovalDecisionInput<'_>,
        ) -> codex_extension_api::ExtensionFuture<'a, Option<ApprovalDecision>> {
            Box::pin(async move {
                assert_eq!(input.action["cwd"], serde_json::json!(self.cwd));
                Some(self.decision.clone())
            })
        }
    }

    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::try_from(PathBuf::from(
        std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xe9".to_vec()),
    ))?);
    let (mut session, turn, events) = make_session_and_context_with_rx().await;
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.approval_review_contributor(Arc::new(Contributor {
        cwd: codex_utils_path_uri::LegacyAppPathString::from_path_uri(&cwd, PathConvention::Posix)?,
        decision,
    }));
    Arc::get_mut(&mut session)
        .context("session is uniquely owned")?
        .services
        .extensions = Arc::new(extensions.build());
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());
    let mut review_context = GuardianReviewContext::from(&turn);
    review_context.approval_policy = AskForApproval::OnRequest;
    review_context.approvals_reviewer = reviewer;
    let context = ApprovalContext {
        review_context,
        cancellation_token: None,
        call_id: "non-utf8-cwd".to_string(),
        tool_name: ToolName::plain("exec_command"),
        strict_auto_review: false,
        approval_reason: None,
        retry_reason: None,
        network_approval_context: None,
    };
    let action = ApprovalAction::ExecCommand {
        id: context.call_id.clone(),
        environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
        command: vec!["npm".to_string(), "install".to_string()],
        hook_command: "npm install".to_string(),
        cwd: cwd.clone(),
        sandbox_permissions: if reviewer == ApprovalsReviewer::User {
            SandboxPermissions::RequireEscalated
        } else {
            SandboxPermissions::UseDefault
        },
        additional_permissions: None,
        justification: None,
        tty: false,
        proposed_execpolicy_amendment: None,
    };
    let approval = session.request_reviewer_approval(action, &context);
    tokio::pin!(approval);
    let expected = if reviewer == ApprovalsReviewer::User {
        tokio::select! {
            resolution = &mut approval => panic!("expected a user prompt, got {resolution:?}"),
            event = events.recv() => {
                let codex_protocol::protocol::EventMsg::ExecApprovalRequest(request) =
                    event.context("receive user prompt")?.msg
                else {
                    panic!("expected a command approval prompt");
                };
                assert_eq!(request.cwd, codex_utils_path_uri::LegacyAppPathString::from(cwd));
                assert_eq!(request.command, vec!["npm", "install"]);
                session.notify_approval(&request.call_id, ReviewDecision::Approved).await;
            }
        }
        ApprovalResolution {
            decision: ReviewDecision::Approved,
            source: ApprovalResolutionSource::User,
        }
    } else {
        ApprovalResolution {
            decision: ReviewDecision::Approved,
            source: ApprovalResolutionSource::Guardian,
        }
    };
    assert_eq!(approval.await, expected);
    assert!(events.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn patch_approval_with_same_reason_reuses_session_decision() -> anyhow::Result<()> {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());
    let mut review_context = GuardianReviewContext::from(&turn);
    review_context.approval_policy = AskForApproval::OnRequest;
    review_context.approvals_reviewer = ApprovalsReviewer::User;
    let file = PathUri::from_abs_path(&AbsolutePathBuf::try_from(PathBuf::from(
        "/tmp/jaimesh-approval-cache.txt",
    ))?);
    let make_action = |id: &str| ApprovalAction::ApplyPatch {
        id: id.to_string(),
        environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: file.clone(),
        files: vec![file.clone()],
        patch: String::new(),
        changes: Arc::new(HashMap::new()),
        permissions_preapproved: false,
    };
    let make_context = |id: &str, reason: &str| ApprovalContext {
        review_context: review_context.clone(),
        cancellation_token: None,
        call_id: id.to_string(),
        tool_name: ToolName::plain("apply_patch"),
        strict_auto_review: false,
        approval_reason: Some(reason.to_string()),
        retry_reason: None,
        network_approval_context: None,
    };

    let first_action = make_action("patch-1");
    let first_context = make_context("patch-1", "outside sandbox");
    let first = session.request_user_approval(&first_action, &first_context);
    tokio::pin!(first);
    tokio::select! {
        result = &mut first => panic!("expected first approval request, got {result:?}"),
        event = events.recv() => {
            let codex_protocol::protocol::EventMsg::ApplyPatchApprovalRequest(request) = event.expect("first approval event").msg else {
                panic!("expected patch approval request");
            };
            session.notify_approval(&request.call_id, ReviewDecision::ApprovedForSession).await;
        }
    }
    assert_eq!(first.await, ReviewDecision::ApprovedForSession);
    let repeated_action = make_action("patch-2");
    let repeated_context = make_context("patch-2", "outside sandbox");
    let repeated = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        session.request_user_approval(&repeated_action, &repeated_context),
    )
    .await
    .expect("same file and reason should reuse session approval");
    assert_eq!(repeated, ReviewDecision::ApprovedForSession);
    assert!(events.try_recv().is_err());

    let changed_action = make_action("patch-3");
    let changed_context = make_context("patch-3", "new permission reason");
    let changed_reason = session.request_user_approval(&changed_action, &changed_context);
    tokio::pin!(changed_reason);
    tokio::select! {
        result = &mut changed_reason => panic!("different reason should prompt, got {result:?}"),
        event = events.recv() => {
            let codex_protocol::protocol::EventMsg::ApplyPatchApprovalRequest(request) = event.expect("second approval event").msg else {
                panic!("expected another patch approval request");
            };
            session.notify_approval(&request.call_id, ReviewDecision::Approved).await;
        }
    }
    assert_eq!(changed_reason.await, ReviewDecision::Approved);
    Ok(())
}

#[tokio::test]
async fn explicit_mcp_reviewer_override_takes_precedence_over_action_context() {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    let action = ApprovalAction::McpToolCall {
        id: "mcp-override".to_string(),
        server: "example".to_string(),
        tool_name: "dangerous".to_string(),
        arguments: None,
        connector_id: None,
        connector_name: None,
        connector_description: None,
        connected_account_email: None,
        tool_title: None,
        tool_description: None,
        annotations: None,
        hook_tool_name: HookToolName::new("mcp__example__dangerous"),
        approval_policy: AskForApproval::OnRequest,
        reviewer: ApprovalsReviewer::User,
        approval_mode: AppToolApproval::Prompt,
        allow_session_remember: false,
        allow_persistent_approval: false,
    };
    let mut review_context = GuardianReviewContext::from(&turn);
    review_context.approval_policy = AskForApproval::OnRequest;
    review_context.approvals_reviewer = ApprovalsReviewer::AutoReview;
    let context = ApprovalContext {
        review_context,
        cancellation_token: None,
        call_id: "mcp-override".to_string(),
        tool_name: ToolName::plain("dangerous"),
        strict_auto_review: false,
        approval_reason: None,
        retry_reason: None,
        network_approval_context: None,
    };

    tokio::select! {
        resolution = session.request_reviewer_approval(action, &context) => {
            panic!("expected a user approval request, got {resolution:?}");
        }
        event = events.recv() => {
            let codex_protocol::protocol::EventMsg::ElicitationRequest(request) =
                event.expect("receive user approval request").msg
            else {
                panic!("expected an MCP user approval request");
            };
            assert_eq!(request.server_name, "example");
            assert_eq!(
                request.id,
                codex_protocol::mcp::RequestId::String(
                    "mcp_tool_call_approval_mcp-override".to_string()
                )
            );
        }
    }
}
