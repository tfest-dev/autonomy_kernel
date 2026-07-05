use crate::cli::{CliError, CliScenarioSummary};

pub fn render_scenario_list(scenarios: &[&str]) -> String {
    let mut output = String::from("supported_scenarios:\n");
    for scenario in scenarios {
        output.push_str("- ");
        output.push_str(scenario);
        output.push('\n');
    }
    output
}

pub fn render_scenario_summary(summary: &CliScenarioSummary) -> String {
    let mut output = String::new();
    output.push_str("scenario: ");
    output.push_str(summary.scenario_name);
    output.push('\n');
    output.push_str("status: ok\n");
    output.push_str("replay_verified: ");
    output.push_str(bool_label(summary.replay_verified));
    output.push('\n');
    output.push_str("events: ");
    output.push_str(&summary.event_count.to_string());
    output.push('\n');
    output.push_str("final_tick: ");
    output.push_str(&summary.final_tick.value().to_string());
    output.push('\n');
    output.push_str("objective_satisfied: ");
    output.push_str(bool_label(summary.objective_satisfied));
    output.push('\n');
    output
}

pub fn render_artifact_export(artifact: &str) -> String {
    artifact.to_string()
}

pub fn render_error(error: &CliError) -> String {
    let mut output = String::from("error: ");
    output.push_str(&error.to_string());
    output
}

fn bool_label(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

#[cfg(test)]
mod tests {
    use autonomy_core::Tick;

    use crate::{
        cli::{CliError, CliScenarioSummary},
        output::{render_error, render_scenario_summary},
    };

    #[test]
    fn error_output_is_stable() {
        assert_eq!(
            render_error(&CliError::UnsupportedScenario("missing".to_string())),
            "error: unsupported scenario 'missing'"
        );
    }

    #[test]
    fn summary_output_is_stable() {
        let summary = CliScenarioSummary {
            scenario_name: "scheduled-mining",
            replay_verified: true,
            event_count: 14,
            final_tick: Tick::new(2),
            objective_satisfied: true,
        };

        assert_eq!(
            render_scenario_summary(&summary),
            "scenario: scheduled-mining\nstatus: ok\nreplay_verified: true\nevents: 14\nfinal_tick: 2\nobjective_satisfied: true\n"
        );
    }
}
