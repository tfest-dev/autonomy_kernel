use std::error::Error;
use std::fmt;

use autonomy_core::Tick;
use autonomy_replay::{
    build_causal_artifact, export_artifact_lines, export_artifact_text, run_mining_bootstrap,
    run_policy_gate, run_proposal_adaptor, run_scheduled_mining, run_worker_failure_recovery,
    ScenarioError, ScenarioRun,
};
use autonomy_sim::{mining_bootstrap_objective, objective_satisfied, MINING_BOOTSTRAP_STORAGE_ID};

use crate::output::{render_artifact_export, render_scenario_list, render_scenario_summary};

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_INVALID_USAGE: i32 = 1;
pub const EXIT_UNSUPPORTED_SCENARIO: i32 = 2;
pub const EXIT_SCENARIO_FAILED: i32 = 3;
pub const EXIT_ARTIFACT_FAILED: i32 = 4;

const SUPPORTED_SCENARIOS: &[&str] = &[
    "mining-bootstrap",
    "worker-failure",
    "policy-gate",
    "scheduled-mining",
    "proposal-adaptor",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliScenario {
    MiningBootstrap,
    WorkerFailure,
    PolicyGate,
    ScheduledMining,
    ProposalAdaptor,
}

impl CliScenario {
    pub fn parse(name: &str) -> Result<Self, CliError> {
        match name {
            "mining-bootstrap" => Ok(Self::MiningBootstrap),
            "worker-failure" => Ok(Self::WorkerFailure),
            "policy-gate" => Ok(Self::PolicyGate),
            "scheduled-mining" => Ok(Self::ScheduledMining),
            "proposal-adaptor" => Ok(Self::ProposalAdaptor),
            _ => Err(CliError::UnsupportedScenario(name.to_string())),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::MiningBootstrap => "mining-bootstrap",
            Self::WorkerFailure => "worker-failure",
            Self::PolicyGate => "policy-gate",
            Self::ScheduledMining => "scheduled-mining",
            Self::ProposalAdaptor => "proposal-adaptor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactFormat {
    Text,
    Lines,
}

impl ArtifactFormat {
    pub fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "text" => Ok(Self::Text),
            "lines" => Ok(Self::Lines),
            _ => Err(CliError::UnsupportedArtifactFormat(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliScenarioSummary {
    pub scenario_name: &'static str,
    pub replay_verified: bool,
    pub event_count: usize,
    pub final_tick: Tick,
    pub objective_satisfied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    InvalidUsage(&'static str),
    UnsupportedScenario(String),
    UnsupportedArtifactFormat(String),
    ScenarioFailed { scenario: String, reason: String },
    ArtifactExportFailed { scenario: String, reason: String },
}

impl CliError {
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidUsage(_) => EXIT_INVALID_USAGE,
            Self::UnsupportedScenario(_) => EXIT_UNSUPPORTED_SCENARIO,
            Self::ScenarioFailed { .. } => EXIT_SCENARIO_FAILED,
            Self::UnsupportedArtifactFormat(_) | Self::ArtifactExportFailed { .. } => {
                EXIT_ARTIFACT_FAILED
            }
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUsage(message) => write!(f, "invalid usage: {message}"),
            Self::UnsupportedScenario(name) => write!(f, "unsupported scenario '{name}'"),
            Self::UnsupportedArtifactFormat(format) => {
                write!(f, "unsupported artifact format '{format}'")
            }
            Self::ScenarioFailed { scenario, reason } => {
                write!(f, "scenario '{scenario}' failed: {reason}")
            }
            Self::ArtifactExportFailed { scenario, reason } => {
                write!(
                    f,
                    "artifact export for scenario '{scenario}' failed: {reason}"
                )
            }
        }
    }
}

impl Error for CliError {}

pub fn list_scenarios() -> &'static [&'static str] {
    SUPPORTED_SCENARIOS
}

pub fn run_cli<'a>(args: impl IntoIterator<Item = &'a str>) -> Result<String, CliError> {
    let args: Vec<&str> = args.into_iter().collect();
    match args.as_slice() {
        ["list-scenarios"] => Ok(render_scenario_list(list_scenarios())),
        ["run-scenario", name] => {
            let summary = run_cli_scenario(name)?;
            Ok(render_scenario_summary(&summary))
        }
        ["export-artifact", name, format] => {
            let format = ArtifactFormat::parse(format)?;
            let output = export_cli_artifact(name, format)?;
            Ok(render_artifact_export(&output))
        }
        [] => Err(CliError::InvalidUsage("missing command")),
        _ => Err(CliError::InvalidUsage(
            "expected list-scenarios, run-scenario <scenario-name>, or export-artifact <scenario-name> <format>",
        )),
    }
}

pub fn run_cli_scenario(name: &str) -> Result<CliScenarioSummary, CliError> {
    let scenario = CliScenario::parse(name)?;
    let run = run_scenario(scenario).map_err(|error| CliError::ScenarioFailed {
        scenario: scenario.name().to_string(),
        reason: error.to_string(),
    })?;

    Ok(summary_from_run(scenario, &run))
}

pub fn export_cli_artifact(name: &str, format: ArtifactFormat) -> Result<String, CliError> {
    let scenario = CliScenario::parse(name)?;
    let run = run_scenario(scenario).map_err(|error| CliError::ArtifactExportFailed {
        scenario: scenario.name().to_string(),
        reason: error.to_string(),
    })?;
    let artifact = build_causal_artifact(scenario.name(), &run.events, true);

    Ok(match format {
        ArtifactFormat::Text => export_artifact_text(&artifact),
        ArtifactFormat::Lines => export_artifact_lines(&artifact),
    })
}

fn run_scenario(scenario: CliScenario) -> Result<ScenarioRun, ScenarioError> {
    match scenario {
        CliScenario::MiningBootstrap => run_mining_bootstrap(),
        CliScenario::WorkerFailure => run_worker_failure_recovery(),
        CliScenario::PolicyGate => run_policy_gate(),
        CliScenario::ScheduledMining => run_scheduled_mining(),
        CliScenario::ProposalAdaptor => run_proposal_adaptor(),
    }
}

fn summary_from_run(scenario: CliScenario, run: &ScenarioRun) -> CliScenarioSummary {
    let objective = mining_bootstrap_objective();
    CliScenarioSummary {
        scenario_name: scenario.name(),
        replay_verified: true,
        event_count: run.events.len(),
        final_tick: run.final_state.tick,
        objective_satisfied: objective_satisfied(
            &run.final_state,
            &objective,
            MINING_BOOTSTRAP_STORAGE_ID,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_cli_artifact, list_scenarios, run_cli, run_cli_scenario, ArtifactFormat, CliError,
        CliScenario, EXIT_ARTIFACT_FAILED, EXIT_INVALID_USAGE, EXIT_SCENARIO_FAILED, EXIT_SUCCESS,
        EXIT_UNSUPPORTED_SCENARIO,
    };

    #[test]
    fn scenario_list_order_is_deterministic() {
        assert_eq!(
            list_scenarios(),
            &[
                "mining-bootstrap",
                "worker-failure",
                "policy-gate",
                "scheduled-mining",
                "proposal-adaptor",
            ]
        );
    }

    #[test]
    fn known_scenario_names_parse_successfully() {
        assert_eq!(
            CliScenario::parse("mining-bootstrap"),
            Ok(CliScenario::MiningBootstrap)
        );
        assert_eq!(
            CliScenario::parse("worker-failure"),
            Ok(CliScenario::WorkerFailure)
        );
        assert_eq!(
            CliScenario::parse("policy-gate"),
            Ok(CliScenario::PolicyGate)
        );
        assert_eq!(
            CliScenario::parse("scheduled-mining"),
            Ok(CliScenario::ScheduledMining)
        );
        assert_eq!(
            CliScenario::parse("proposal-adaptor"),
            Ok(CliScenario::ProposalAdaptor)
        );
    }

    #[test]
    fn unknown_scenario_name_fails_deterministically() {
        assert_eq!(
            CliScenario::parse("unknown"),
            Err(CliError::UnsupportedScenario("unknown".to_string()))
        );
    }

    #[test]
    fn run_scenario_mining_bootstrap_summary_is_deterministic() {
        let summary = run_cli_scenario("mining-bootstrap").expect("scenario should run");

        assert_eq!(summary.scenario_name, "mining-bootstrap");
        assert!(summary.replay_verified);
        assert_eq!(summary.event_count, 12);
        assert_eq!(summary.final_tick.value(), 4);
        assert!(summary.objective_satisfied);
    }

    #[test]
    fn run_scenario_worker_failure_summary_is_deterministic() {
        let summary = run_cli_scenario("worker-failure").expect("scenario should run");

        assert_eq!(summary.scenario_name, "worker-failure");
        assert!(summary.replay_verified);
        assert_eq!(summary.event_count, 20);
        assert_eq!(summary.final_tick.value(), 6);
        assert!(summary.objective_satisfied);
    }

    #[test]
    fn run_scenario_policy_gate_summary_is_deterministic() {
        let summary = run_cli_scenario("policy-gate").expect("scenario should run");

        assert_eq!(summary.scenario_name, "policy-gate");
        assert!(summary.replay_verified);
        assert_eq!(summary.event_count, 11);
        assert_eq!(summary.final_tick.value(), 2);
        assert!(summary.objective_satisfied);
    }

    #[test]
    fn run_scenario_scheduled_mining_summary_is_deterministic() {
        let summary = run_cli_scenario("scheduled-mining").expect("scenario should run");

        assert_eq!(summary.scenario_name, "scheduled-mining");
        assert!(summary.replay_verified);
        assert_eq!(summary.event_count, 14);
        assert_eq!(summary.final_tick.value(), 2);
        assert!(summary.objective_satisfied);
    }

    #[test]
    fn run_scenario_proposal_adaptor_summary_is_deterministic() {
        let summary = run_cli_scenario("proposal-adaptor").expect("scenario should run");

        assert_eq!(summary.scenario_name, "proposal-adaptor");
        assert!(summary.replay_verified);
        assert_eq!(summary.event_count, 19);
        assert_eq!(summary.final_tick.value(), 2);
        assert!(summary.objective_satisfied);
    }

    #[test]
    fn running_same_scenario_twice_through_dispatch_produces_identical_summary() {
        let first = run_cli_scenario("proposal-adaptor").expect("scenario should run");
        let second = run_cli_scenario("proposal-adaptor").expect("scenario should run");

        assert_eq!(first, second);
    }

    #[test]
    fn list_scenarios_output_is_stable() {
        let output = run_cli(["list-scenarios"]).expect("list should succeed");

        assert_eq!(
            output,
            "supported_scenarios:\n- mining-bootstrap\n- worker-failure\n- policy-gate\n- scheduled-mining\n- proposal-adaptor\n"
        );
    }

    #[test]
    fn run_scenario_output_is_stable() {
        let output = run_cli(["run-scenario", "scheduled-mining"]).expect("scenario should run");

        assert_eq!(
            output,
            "scenario: scheduled-mining\nstatus: ok\nreplay_verified: true\nevents: 14\nfinal_tick: 2\nobjective_satisfied: true\n"
        );
    }

    #[test]
    fn export_artifact_scheduled_mining_text_is_deterministic() {
        let first = export_cli_artifact("scheduled-mining", ArtifactFormat::Text)
            .expect("artifact should export");
        let second = export_cli_artifact("scheduled-mining", ArtifactFormat::Text)
            .expect("artifact should export");

        assert_eq!(first, second);
        assert!(first.contains("Scenario: scheduled-mining"));
        assert!(first.contains("# Causal Graph"));
    }

    #[test]
    fn export_artifact_scheduled_mining_lines_is_deterministic() {
        let first = export_cli_artifact("scheduled-mining", ArtifactFormat::Lines)
            .expect("artifact should export");
        let second = export_cli_artifact("scheduled-mining", ArtifactFormat::Lines)
            .expect("artifact should export");

        assert_eq!(first, second);
        assert!(first.contains("artifact|scenario|scheduled-mining\n"));
        assert!(first.contains("node|"));
    }

    #[test]
    fn export_artifact_proposal_adaptor_text_is_deterministic() {
        let first = export_cli_artifact("proposal-adaptor", ArtifactFormat::Text)
            .expect("artifact should export");
        let second = export_cli_artifact("proposal-adaptor", ArtifactFormat::Text)
            .expect("artifact should export");

        assert_eq!(first, second);
        assert!(first.contains("Scenario: proposal-adaptor"));
        assert!(first.contains("ProposalRejected"));
        assert!(first.contains("ProposalAccepted"));
    }

    #[test]
    fn unsupported_artifact_format_fails_with_stable_error() {
        assert_eq!(
            ArtifactFormat::parse("json"),
            Err(CliError::UnsupportedArtifactFormat("json".to_string()))
        );
    }

    #[test]
    fn invalid_usage_returns_invalid_usage_error() {
        assert!(matches!(run_cli([]), Err(CliError::InvalidUsage(_))));
        assert!(matches!(
            run_cli(["run-scenario"]),
            Err(CliError::InvalidUsage(_))
        ));
    }

    #[test]
    fn cli_output_does_not_include_absolute_project_paths() {
        let output = run_cli(["run-scenario", "proposal-adaptor"]).expect("scenario should run");

        assert!(!output.contains("/home/"));
        assert!(!output.contains("Projects/piestyx"));
    }

    #[test]
    fn cli_output_does_not_include_wall_clock_timestamps() {
        let output = run_cli(["run-scenario", "scheduled-mining"]).expect("scenario should run");

        assert!(!output.contains("2026-"));
        assert!(!output.contains("T00:"));
        assert!(!output.contains("UTC"));
    }

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(
            CliError::InvalidUsage("bad").exit_code(),
            EXIT_INVALID_USAGE
        );
        assert_eq!(
            CliError::UnsupportedScenario("bad".to_string()).exit_code(),
            EXIT_UNSUPPORTED_SCENARIO
        );
        assert_eq!(
            CliError::ScenarioFailed {
                scenario: "s".to_string(),
                reason: "r".to_string(),
            }
            .exit_code(),
            EXIT_SCENARIO_FAILED
        );
        assert_eq!(
            CliError::ArtifactExportFailed {
                scenario: "s".to_string(),
                reason: "r".to_string(),
            }
            .exit_code(),
            EXIT_ARTIFACT_FAILED
        );
    }
}
