defmodule AutonomyKernelBeam.ScenarioWorkerTest do
  use ExUnit.Case, async: false

  alias AutonomyKernelBeam.Messages.ExportArtifactRequest
  alias AutonomyKernelBeam.Messages.ExportArtifactResponse
  alias AutonomyKernelBeam.Messages.RunScenarioRequest
  alias AutonomyKernelBeam.Messages.RunScenarioResponse
  alias AutonomyKernelBeam.ScenarioWorker

  @worker_name __MODULE__.Worker
  @scheduled_mining_output """
  scenario: scheduled-mining
  status: ok
  replay_verified: true
  events: 14
  final_tick: 2
  objective_satisfied: true
  """

  setup do
    test_pid = self()

    runner = fn command, args, opts ->
      send(test_pid, {:command_invoked, command, args, opts})
      {@scheduled_mining_output, 0}
    end

    start_supervised!(
      {ScenarioWorker, name: @worker_name, rust_cli_opts: [command_runner: runner]}
    )

    :ok
  end

  test "runs supported scenario request through Rust CLI boundary" do
    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: "request-1"
    }

    assert %RunScenarioResponse{
             request_id: "request-1",
             status: :ok,
             scenario_name: "scheduled-mining",
             replay_verified: true,
             events: 14,
             final_tick: 2,
             objective_satisfied: true
           } = ScenarioWorker.run_scenario(@worker_name, request)

    assert_receive {:command_invoked, "cargo", args, _opts}
    assert args == ["run", "-p", "autonomy-cli", "--", "run-scenario", "scheduled-mining"]
  end

  test "rejects unsupported scenario request" do
    request = %RunScenarioRequest{
      scenario_name: "unknown-scenario",
      request_id: "request-2"
    }

    assert %RunScenarioResponse{
             request_id: "request-2",
             status: :unsupported_scenario,
             scenario_name: "unknown-scenario"
           } = ScenarioWorker.run_scenario(@worker_name, request)

    refute_receive {:command_invoked, _command, _args, _opts}
  end

  test "preserves request ID and returns deterministic response for same request" do
    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: 42
    }

    first = ScenarioWorker.run_scenario(@worker_name, request)
    second = ScenarioWorker.run_scenario(@worker_name, request)

    assert first == second
    assert first.request_id == 42
    assert first.status == :ok
  end

  test "does not require generated request IDs" do
    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: {:explicit, 7}
    }

    response = ScenarioWorker.run_scenario(@worker_name, request)

    assert response.request_id == {:explicit, 7}
  end

  test "exports artifact through Rust CLI boundary" do
    request = %ExportArtifactRequest{
      scenario_name: "scheduled-mining",
      format: "lines",
      request_id: "artifact-1"
    }

    assert %ExportArtifactResponse{
             request_id: "artifact-1",
             status: :ok,
             scenario_name: "scheduled-mining",
             format: "lines",
             artifact: @scheduled_mining_output
           } = ScenarioWorker.export_artifact(@worker_name, request)

    assert_receive {:command_invoked, "cargo", args, _opts}

    assert args == [
             "run",
             "-p",
             "autonomy-cli",
             "--",
             "export-artifact",
             "scheduled-mining",
             "lines"
           ]
  end

  test "rejects unsupported artifact format without Rust invocation" do
    request = %ExportArtifactRequest{
      scenario_name: "scheduled-mining",
      format: "json",
      request_id: "artifact-2"
    }

    assert %ExportArtifactResponse{
             request_id: "artifact-2",
             status: :unsupported_artifact_format,
             scenario_name: "scheduled-mining",
             format: "json"
           } = ScenarioWorker.export_artifact(@worker_name, request)

    refute_receive {:command_invoked, _command, _args, _opts}
  end
end
