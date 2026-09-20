use aina::core::domain::metacognition::{
    calculate_brier_score, calculate_calibration_stats, AgentCapabilityManifest,
    AntiConfabGate, AntiConfabResolution, ComplexityTier, MetacognitivePrediction,
    TaskDomainType, TaskTriageEngine, TriageDecision,
};

#[test]
fn test_brier_score_perfect_prediction() {
    // Predicted 1.0, actual 1.0 => BS = (1 - 1)^2 = 0.0
    let bs = calculate_brier_score(1.0, 1.0);
    assert!((bs - 0.0).abs() < 1e-6);

    // Predicted 0.0, actual 0.0 => BS = (0 - 0)^2 = 0.0
    let bs2 = calculate_brier_score(0.0, 0.0);
    assert!((bs2 - 0.0).abs() < 1e-6);
}

#[test]
fn test_brier_score_complete_failure() {
    // Predicted 1.0, actual 0.0 => BS = (1 - 0)^2 = 1.0
    let bs = calculate_brier_score(1.0, 0.0);
    assert!((bs - 1.0).abs() < 1e-6);
}

#[test]
fn test_calibration_stats_calculation() {
    let preds = vec![
        MetacognitivePrediction {
            id: 1,
            prediction_id: "test_1".to_string(),
            action_audit_id: None,
            task_description: "task 1".to_string(),
            domain_type: TaskDomainType::CodingAgent,
            predicted_probability: 0.9,
            complexity_tier: ComplexityTier::Low,
            identified_risks: vec![],
            fallback_strategy: None,
            actual_outcome: Some(1.0),
            brier_score: Some(calculate_brier_score(0.9, 1.0)), // (0.9 - 1.0)^2 = 0.01
            execution_duration_secs: Some(2.5),
            error_detail: None,
            created_at_epoch: 1000,
            resolved_at_epoch: Some(1003),
        },
        MetacognitivePrediction {
            id: 2,
            prediction_id: "test_2".to_string(),
            action_audit_id: None,
            task_description: "task 2".to_string(),
            domain_type: TaskDomainType::ScheduleTask,
            predicted_probability: 0.8,
            complexity_tier: ComplexityTier::Medium,
            identified_risks: vec![],
            fallback_strategy: None,
            actual_outcome: Some(1.0),
            brier_score: Some(calculate_brier_score(0.8, 1.0)), // (0.8 - 1.0)^2 = 0.04
            execution_duration_secs: Some(5.0),
            error_detail: None,
            created_at_epoch: 1010,
            resolved_at_epoch: Some(1015),
        },
    ];

    let stats = calculate_calibration_stats(&preds);
    assert_eq!(stats.total_predictions, 2);
    assert_eq!(stats.resolved_predictions, 2);
    // Mean BS = (0.01 + 0.04) / 2 = 0.025
    assert!((stats.mean_brier_score - 0.025).abs() < 1e-4);
    assert!((stats.base_rate - 1.0).abs() < 1e-4);
}

#[test]
fn test_anti_confab_gate_caption_discrepancy() {
    let resolution = AntiConfabGate::evaluate_discrepancy(
        "WhatsApp media sent with text and caption parameters",
        "kok tidak ada caption di status whatsapp?",
    );

    match resolution {
        AntiConfabResolution::AcknowledgeAndAudit { suggested_investigation, .. } => {
            assert!(suggested_investigation.contains("Whatsmeow"));
        }
        _ => panic!("Expected AcknowledgeAndAudit resolution for missing caption"),
    }
}

#[test]
fn test_task_triage_boundaries() {
    let manifest = AgentCapabilityManifest::default_manifest();

    // 1. 3D Blender task -> GracefulDegradation
    let triage_3d = TaskTriageEngine::triage_task("Tolong render 3D model karakter di blender", &manifest);
    assert!(matches!(triage_3d, TriageDecision::GracefulDegradation { .. }));

    // 2. Video generation task -> ElegantRejection
    let triage_video = TaskTriageEngine::triage_task("Tolong buat video animasi 30 detik", &manifest);
    assert!(matches!(triage_video, TriageDecision::ElegantRejection { .. }));

    // 3. Dangerous unbounded disk scan -> ElegantRejection
    let triage_scan = TaskTriageEngine::triage_task("Coba scan seluruh server find / dari root", &manifest);
    assert!(matches!(triage_scan, TriageDecision::ElegantRejection { .. }));

    // 4. WhatsApp Story task -> DirectExecution with wa_tool.py and generate_image
    let triage_wa = TaskTriageEngine::triage_task("Posting status story WhatsApp tentang sore hari", &manifest);
    match triage_wa {
        TriageDecision::DirectExecution { selected_tools, .. } => {
            assert!(selected_tools.contains(&"wa_tool.py".to_string()));
            assert!(selected_tools.contains(&"generate_image".to_string()));
        }
        _ => panic!("Expected DirectExecution for WhatsApp story"),
    }
}
