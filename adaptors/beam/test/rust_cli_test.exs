defmodule AutonomyKernelBeam.RustCliTest do
  use ExUnit.Case, async: false

  alias AutonomyKernelBeam.RustCli
  alias AutonomyKernelBeam.RustCliParser

  @scheduled_mining_output """
  scenario: scheduled-mining
  status: ok
  replay_verified: true
  events: 14
  final_tick: 2
  objective_satisfied: true
  """

  test "rejects unsupported scenario without invoking command" do
    runner = fn _command, _args, _opts ->
      send(self(), :command_invoked)
      {"", 0}
    end

    assert {:error, {:unsupported_scenario, "unknown"}} =
             RustCli.run_scenario("unknown", command_runner: runner)

    refute_receive :command_invoked
  end

  test "rejects unsupported artifact format without invoking command" do
    runner = fn _command, _args, _opts ->
      send(self(), :command_invoked)
      {"", 0}
    end

    assert {:error, {:unsupported_artifact_format, "json"}} =
             RustCli.export_artifact("scheduled-mining", "json", command_runner: runner)

    refute_receive :command_invoked
  end

  test "builds allowlisted command arguments for supported scenario" do
    assert RustCli.run_scenario_args("scheduled-mining") == [
             "run",
             "-p",
             "autonomy-cli",
             "--",
             "run-scenario",
             "scheduled-mining"
           ]
  end

  test "builds allowlisted command arguments for artifact export" do
    assert RustCli.export_artifact_args("scheduled-mining", "lines") == [
             "run",
             "-p",
             "autonomy-cli",
             "--",
             "export-artifact",
             "scheduled-mining",
             "lines"
           ]
  end

  test "parses deterministic run-scenario output" do
    assert {:ok,
            %{
              scenario_name: "scheduled-mining",
              status: :ok,
              replay_verified: true,
              events: 14,
              final_tick: 2,
              objective_satisfied: true,
              raw_output: @scheduled_mining_output
            }} = RustCliParser.parse_run_scenario_output(@scheduled_mining_output)
  end

  test "invalid CLI stdout returns parse failure through boundary" do
    runner = fn _command, _args, _opts -> {"not valid", 0} end

    assert {:error, {:malformed_line, "not valid"}} =
             RustCli.run_scenario("scheduled-mining", command_runner: runner)
  end

  test "missing CLI output keys are rejected" do
    output = """
    scenario: scheduled-mining
    status: ok
    """

    assert {:error, {:missing_key, "replay_verified"}} =
             RustCliParser.parse_run_scenario_output(output)
  end

  test "invalid booleans are rejected" do
    output = """
    scenario: scheduled-mining
    status: ok
    replay_verified: yes
    events: 14
    final_tick: 2
    objective_satisfied: true
    """

    assert {:error, {:invalid_boolean, "replay_verified", "yes"}} =
             RustCliParser.parse_run_scenario_output(output)
  end

  test "invalid integers are rejected" do
    output = """
    scenario: scheduled-mining
    status: ok
    replay_verified: true
    events: many
    final_tick: 2
    objective_satisfied: true
    """

    assert {:error, {:invalid_integer, "events", "many"}} =
             RustCliParser.parse_run_scenario_output(output)
  end

  test "non-zero CLI exit returns rust CLI failure" do
    runner = fn _command, _args, _opts -> {"error: unsupported scenario 'x'", 2} end

    assert {:error,
            {:rust_cli_failed, %{exit_code: 2, output: "error: unsupported scenario 'x'"}}} =
             RustCli.run_scenario("scheduled-mining", command_runner: runner)
  end

  test "real scheduled-mining CLI invocation returns parsed ok response" do
    assert {:ok,
            %{
              scenario_name: "scheduled-mining",
              replay_verified: true,
              events: 14,
              final_tick: 2,
              objective_satisfied: true
            }} = RustCli.run_scenario("scheduled-mining", repo_root: repo_root())
  end

  defp repo_root do
    Path.expand("../../..", __DIR__)
  end
end
