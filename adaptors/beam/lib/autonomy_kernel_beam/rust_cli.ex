defmodule AutonomyKernelBeam.RustCli do
  @moduledoc """
  Controlled process boundary for invoking the deterministic Rust CLI.

  This module allowlists scenario names and artifact formats before invoking
  `cargo run -p autonomy-cli -- ...`. It uses `System.cmd/3` with argument
  lists only and does not execute shell strings.
  """

  alias AutonomyKernelBeam.RustCliParser

  @supported_scenarios [
    "mining-bootstrap",
    "worker-failure",
    "policy-gate",
    "scheduled-mining",
    "proposal-adapter"
  ]

  @supported_artifact_formats ["text", "lines"]
  @default_repo_root Path.expand("../../../..", __DIR__)

  def supported_scenarios, do: @supported_scenarios

  def supported_scenario?(scenario_name), do: scenario_name in @supported_scenarios

  def supported_artifact_format?(format), do: format in @supported_artifact_formats

  def list_scenarios(opts \\ []) do
    run_cli(list_scenarios_args(), opts)
  end

  def run_scenario(scenario_name, opts \\ []) do
    if supported_scenario?(scenario_name) do
      scenario_name
      |> run_scenario_args()
      |> run_cli(opts)
      |> parse_scenario_result()
    else
      {:error, {:unsupported_scenario, scenario_name}}
    end
  end

  def export_artifact(scenario_name, format, opts \\ []) do
    cond do
      not supported_scenario?(scenario_name) ->
        {:error, {:unsupported_scenario, scenario_name}}

      not supported_artifact_format?(format) ->
        {:error, {:unsupported_artifact_format, format}}

      true ->
        export_artifact_args(scenario_name, format)
        |> run_cli(opts)
        |> case do
          {:ok, output} -> {:ok, output}
          error -> error
        end
    end
  end

  def list_scenarios_args, do: ["run", "-p", "autonomy-cli", "--", "list-scenarios"]

  def run_scenario_args(scenario_name) when is_binary(scenario_name) do
    ["run", "-p", "autonomy-cli", "--", "run-scenario", scenario_name]
  end

  def export_artifact_args(scenario_name, format)
      when is_binary(scenario_name) and is_binary(format) do
    ["run", "-p", "autonomy-cli", "--", "export-artifact", scenario_name, format]
  end

  defp parse_scenario_result({:ok, output}), do: RustCliParser.parse_run_scenario_output(output)
  defp parse_scenario_result(error), do: error

  defp run_cli(args, opts) do
    command = Keyword.get(opts, :command, "cargo")
    repo_root = Keyword.get(opts, :repo_root, @default_repo_root)
    runner = Keyword.get(opts, :command_runner, &System.cmd/3)

    case invoke_runner(runner, command, args, cd: repo_root) do
      {:ok, {output, 0}} ->
        {:ok, output}

      {:ok, {output, exit_code}} ->
        {:error, {:rust_cli_failed, %{exit_code: exit_code, output: output}}}

      {:error, reason} ->
        {:error, {:rust_cli_failed, reason}}
    end
  end

  defp invoke_runner(runner, command, args, cmd_opts) do
    {:ok, runner.(command, args, cmd_opts)}
  rescue
    _error -> {:error, %{exit_code: nil, output: "command invocation failed"}}
  end
end
