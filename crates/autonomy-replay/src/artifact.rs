use crate::{
    causal_graph::{
        build_causal_graph, export_causal_graph_lines, export_causal_graph_text, CausalGraph,
    },
    event_log::EventEnvelope,
    scenario::{run_proposal_adaptor, run_scheduled_mining, ScenarioError},
    verification::verify_replay,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalArtifact {
    pub scenario_name: String,
    pub replay_verified: bool,
    pub event_count: usize,
    pub graph: CausalGraph,
}

pub fn build_causal_artifact(
    scenario_name: &str,
    events: &[EventEnvelope],
    replay_verified: bool,
) -> CausalArtifact {
    CausalArtifact {
        scenario_name: scenario_name.to_string(),
        replay_verified,
        event_count: events.len(),
        graph: build_causal_graph(events, replay_verified),
    }
}

pub fn scheduled_mining_causal_artifact() -> Result<CausalArtifact, ScenarioError> {
    let run = run_scheduled_mining()?;
    verify_replay(&run.initial_state, &run.events, &run.final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(build_causal_artifact("scheduled-mining", &run.events, true))
}

pub fn proposal_adaptor_causal_artifact() -> Result<CausalArtifact, ScenarioError> {
    let run = run_proposal_adaptor()?;
    verify_replay(&run.initial_state, &run.events, &run.final_state)
        .map_err(ScenarioError::ReplayFailed)?;

    Ok(build_causal_artifact("proposal-adaptor", &run.events, true))
}

pub fn export_artifact_text(artifact: &CausalArtifact) -> String {
    let mut output = String::new();
    output.push_str("# Causal Artifact\n\n");
    output.push_str("Scenario: ");
    output.push_str(&artifact.scenario_name);
    output.push('\n');
    output.push_str("Replay verified: ");
    output.push_str(bool_label(artifact.replay_verified));
    output.push('\n');
    output.push_str("Event count: ");
    output.push_str(&artifact.event_count.to_string());
    output.push('\n');
    output.push_str("Node count: ");
    output.push_str(&artifact.graph.nodes.len().to_string());
    output.push('\n');
    output.push_str("Edge count: ");
    output.push_str(&artifact.graph.edges.len().to_string());
    output.push_str("\n\n");
    output.push_str(&export_causal_graph_text(&artifact.graph));
    output
}

pub fn export_artifact_lines(artifact: &CausalArtifact) -> String {
    let mut output = String::new();
    output.push_str("artifact|scenario|");
    output.push_str(&escape_line_field(&artifact.scenario_name));
    output.push('\n');
    output.push_str("artifact|replay_verified|");
    output.push_str(bool_label(artifact.replay_verified));
    output.push('\n');
    output.push_str("artifact|event_count|");
    output.push_str(&artifact.event_count.to_string());
    output.push('\n');
    output.push_str("artifact|node_count|");
    output.push_str(&artifact.graph.nodes.len().to_string());
    output.push('\n');
    output.push_str("artifact|edge_count|");
    output.push_str(&artifact.graph.edges.len().to_string());
    output.push('\n');
    output.push_str(&export_causal_graph_lines(&artifact.graph));
    output
}

fn bool_label(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn escape_line_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use crate::{
        artifact::{
            export_artifact_lines, export_artifact_text, proposal_adaptor_causal_artifact,
            scheduled_mining_causal_artifact,
        },
        scenario::run_scheduled_mining,
    };

    #[test]
    fn scheduled_mining_artifact_includes_metadata() {
        let artifact =
            scheduled_mining_causal_artifact().expect("scheduled-mining artifact should build");

        assert_eq!(artifact.scenario_name, "scheduled-mining");
        assert!(artifact.replay_verified);
        assert_eq!(artifact.event_count, 14);
        assert_eq!(artifact.graph.nodes.len(), 15);
        assert!(!artifact.graph.edges.is_empty());
    }

    #[test]
    fn scheduled_mining_artifact_generation_is_deterministic() {
        let first = scheduled_mining_causal_artifact()
            .expect("first scheduled-mining artifact should build");
        let second = scheduled_mining_causal_artifact()
            .expect("second scheduled-mining artifact should build");

        assert_eq!(first, second);
        assert_eq!(export_artifact_text(&first), export_artifact_text(&second));
        assert_eq!(
            export_artifact_lines(&first),
            export_artifact_lines(&second)
        );
    }

    #[test]
    fn artifact_export_contains_required_metadata() {
        let artifact =
            scheduled_mining_causal_artifact().expect("scheduled-mining artifact should build");
        let text = export_artifact_text(&artifact);
        let lines = export_artifact_lines(&artifact);

        assert!(text.contains("Scenario: scheduled-mining"));
        assert!(text.contains("Replay verified: true"));
        assert!(text.contains("Event count: 14"));
        assert!(text.contains("Node count: 15"));
        assert!(text.contains("Edge count:"));

        assert!(lines.contains("artifact|scenario|scheduled-mining\n"));
        assert!(lines.contains("artifact|replay_verified|true\n"));
        assert!(lines.contains("artifact|event_count|14\n"));
        assert!(lines.contains("artifact|node_count|15\n"));
        assert!(lines.contains("artifact|edge_count|"));
    }

    #[test]
    fn artifact_generation_does_not_mutate_scenario_state() {
        let before = run_scheduled_mining().expect("scheduled-mining should run");
        let _artifact =
            scheduled_mining_causal_artifact().expect("scheduled-mining artifact should build");
        let after = run_scheduled_mining().expect("scheduled-mining should still run");

        assert_eq!(before, after);
    }

        #[test]
    fn proposal_adaptor_artifact_generation_is_deterministic() {
        let first =
            proposal_adaptor_causal_artifact().expect("proposal-adaptor artifact should build");
        let second =
            proposal_adaptor_causal_artifact().expect("proposal-adaptor artifact should build");

        assert_eq!(first, second);
        assert_eq!(first.scenario_name, "proposal-adaptor");
        assert!(first.replay_verified);
        assert_eq!(export_artifact_text(&first), export_artifact_text(&second));
        assert_eq!(
            export_artifact_lines(&first),
            export_artifact_lines(&second)
        );
        assert!(export_artifact_lines(&first).contains("artifact|scenario|proposal-adaptor\n"));
    }
}
