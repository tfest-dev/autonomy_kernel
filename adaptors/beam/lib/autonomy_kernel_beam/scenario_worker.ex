defmodule AutonomyKernelBeam.ScenarioWorker do
  @moduledoc """
  Supervised GenServer that accepts deterministic scenario run requests.

  The worker delegates supported execution requests to the controlled Rust CLI
  boundary. It does not own world state, duplicate kernel logic, start
  networking, or write files.
  """

  use GenServer

  alias AutonomyKernelBeam.Messages.ExportArtifactRequest
  alias AutonomyKernelBeam.Messages.ExportArtifactResponse
  alias AutonomyKernelBeam.Messages.RunScenarioRequest
  alias AutonomyKernelBeam.Messages.RunScenarioResponse
  alias AutonomyKernelBeam.RustCli

  def start_link(opts \\ []) do
    name = Keyword.get(opts, :name, __MODULE__)
    rust_cli_opts = Keyword.get(opts, :rust_cli_opts, [])

    GenServer.start_link(__MODULE__, %{handled_requests: 0, rust_cli_opts: rust_cli_opts},
      name: name
    )
  end

  def run_scenario(worker, %RunScenarioRequest{} = request) do
    GenServer.call(worker, {:run_scenario, request})
  end

  def export_artifact(worker, %ExportArtifactRequest{} = request) do
    GenServer.call(worker, {:export_artifact, request})
  end

  @impl true
  def init(state) do
    {:ok, state}
  end

  @impl true
  def handle_call({:run_scenario, request}, _from, state) do
    response = run_scenario_response(request, state.rust_cli_opts)

    {:reply, response, %{state | handled_requests: state.handled_requests + 1}}
  end

  @impl true
  def handle_call({:export_artifact, request}, _from, state) do
    response = export_artifact_response(request, state.rust_cli_opts)

    {:reply, response, %{state | handled_requests: state.handled_requests + 1}}
  end

  defp run_scenario_response(request, rust_cli_opts) do
    case RustCli.run_scenario(request.scenario_name, rust_cli_opts) do
      {:ok, summary} ->
        %RunScenarioResponse{
          request_id: request.request_id,
          status: :ok,
          scenario_name: summary.scenario_name,
          replay_verified: summary.replay_verified,
          events: summary.events,
          final_tick: summary.final_tick,
          objective_satisfied: summary.objective_satisfied,
          raw_output: summary.raw_output
        }

      {:error, {:unsupported_scenario, scenario_name}} ->
        %RunScenarioResponse{
          request_id: request.request_id,
          status: :unsupported_scenario,
          scenario_name: scenario_name,
          error: "unsupported scenario"
        }

      {:error, {:rust_cli_failed, reason}} ->
        %RunScenarioResponse{
          request_id: request.request_id,
          status: :rust_cli_failed,
          scenario_name: request.scenario_name,
          error: inspect(reason)
        }

      {:error, reason} ->
        %RunScenarioResponse{
          request_id: request.request_id,
          status: :parse_failed,
          scenario_name: request.scenario_name,
          error: inspect(reason)
        }
    end
  end

  defp export_artifact_response(request, rust_cli_opts) do
    case RustCli.export_artifact(request.scenario_name, request.format, rust_cli_opts) do
      {:ok, artifact} ->
        %ExportArtifactResponse{
          request_id: request.request_id,
          status: :ok,
          scenario_name: request.scenario_name,
          format: request.format,
          artifact: artifact
        }

      {:error, {:unsupported_scenario, scenario_name}} ->
        %ExportArtifactResponse{
          request_id: request.request_id,
          status: :unsupported_scenario,
          scenario_name: scenario_name,
          format: request.format,
          error: "unsupported scenario"
        }

      {:error, {:unsupported_artifact_format, format}} ->
        %ExportArtifactResponse{
          request_id: request.request_id,
          status: :unsupported_artifact_format,
          scenario_name: request.scenario_name,
          format: format,
          error: "unsupported artifact format"
        }

      {:error, {:rust_cli_failed, reason}} ->
        %ExportArtifactResponse{
          request_id: request.request_id,
          status: :rust_cli_failed,
          scenario_name: request.scenario_name,
          format: request.format,
          error: inspect(reason)
        }
    end
  end
end
