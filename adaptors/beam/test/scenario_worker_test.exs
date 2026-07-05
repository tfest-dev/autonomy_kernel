defmodule AutonomyKernelBeam.ScenarioWorkerTest do
  use ExUnit.Case, async: false

  alias AutonomyKernelBeam.Messages.RunScenarioRequest
  alias AutonomyKernelBeam.Messages.RunScenarioResponse
  alias AutonomyKernelBeam.ScenarioWorker

  @worker_name __MODULE__.Worker

  setup do
    start_supervised!({ScenarioWorker, name: @worker_name})
    :ok
  end

  test "accepts supported scenario request" do
    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: "request-1"
    }

    assert %RunScenarioResponse{
             request_id: "request-1",
             status: :accepted,
             scenario_name: "scheduled-mining"
           } = ScenarioWorker.run_scenario(@worker_name, request)
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
  end

  test "preserves request ID and returns deterministic response for same request" do
    request = %RunScenarioRequest{
      scenario_name: "proposal-adaptor",
      request_id: 42
    }

    first = ScenarioWorker.run_scenario(@worker_name, request)
    second = ScenarioWorker.run_scenario(@worker_name, request)

    assert first == second
    assert first.request_id == 42
    assert first.status == :accepted
  end

  test "does not require generated request IDs" do
    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: {:explicit, 7}
    }

    response = ScenarioWorker.run_scenario(@worker_name, request)

    assert response.request_id == {:explicit, 7}
  end

  test "boundary response does not claim Rust execution" do
    request = %RunScenarioRequest{
      scenario_name: "proposal-adaptor",
      request_id: "boundary-only"
    }

    response = ScenarioWorker.run_scenario(@worker_name, request)

    assert response.status == :accepted
    assert response.note =~ "Rust execution is not performed"
  end
end
