defmodule AutonomyKernelBeam do
  @moduledoc """
  Optional BEAM supervision boundary for deterministic scenario-style requests.

  The adapter invokes the Rust CLI through a controlled process boundary for
  supported scenario requests. It does not own kernel state, evaluate policies,
  run replay, interpret proposal text, or duplicate Rust semantics.
  """

  alias AutonomyKernelBeam.Messages.ExportArtifactRequest
  alias AutonomyKernelBeam.Messages.RunScenarioRequest
  alias AutonomyKernelBeam.ScenarioWorker

  @doc """
  Sends a deterministic scenario request to a supervised scenario worker.
  """
  def run_scenario(%RunScenarioRequest{} = request, worker \\ ScenarioWorker) do
    ScenarioWorker.run_scenario(worker, request)
  end

  @doc """
  Sends a deterministic artifact export request to a supervised scenario worker.
  """
  def export_artifact(%ExportArtifactRequest{} = request, worker \\ ScenarioWorker) do
    ScenarioWorker.export_artifact(worker, request)
  end
end
