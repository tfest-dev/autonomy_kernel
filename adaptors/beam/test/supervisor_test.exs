defmodule AutonomyKernelBeam.SupervisorTest do
  use ExUnit.Case, async: false

  alias AutonomyKernelBeam.Messages.RunScenarioRequest
  alias AutonomyKernelBeam.ScenarioWorker

  test "application supervisor starts scenario worker" do
    assert Process.whereis(AutonomyKernelBeam.Supervisor) |> is_pid()
    assert Process.whereis(ScenarioWorker) |> is_pid()
  end

  test "supervisor restarts scenario worker after crash" do
    old_worker = Process.whereis(ScenarioWorker)
    ref = Process.monitor(old_worker)

    Process.exit(old_worker, :kill)

    assert_receive {:DOWN, ^ref, :process, ^old_worker, :killed}
    new_worker = wait_for_restarted_worker(old_worker)

    assert is_pid(new_worker)
    assert new_worker != old_worker

    request = %RunScenarioRequest{
      scenario_name: "scheduled-mining",
      request_id: "after-restart"
    }

    assert %{status: :accepted, request_id: "after-restart"} =
             ScenarioWorker.run_scenario(ScenarioWorker, request)
  end

  test "adaptor declares no dependencies beyond standard OTP applications" do
    assert AutonomyKernelBeam.MixProject.project()[:deps] == []
    assert AutonomyKernelBeam.MixProject.application()[:extra_applications] == [:logger]
  end

  test "adaptor does not declare networking applications" do
    extra_applications = AutonomyKernelBeam.MixProject.application()[:extra_applications]

    refute :inets in extra_applications
    refute :ssl in extra_applications
  end

  test "adaptor source does not execute shell commands or ports" do
    source =
      "lib/**/*.ex"
      |> Path.wildcard()
      |> Enum.map_join("\n", &File.read!/1)

    refute source =~ "System.cmd"
    refute source =~ "Port.open"
  end

  defp wait_for_restarted_worker(old_worker, attempts \\ 20)

  defp wait_for_restarted_worker(old_worker, attempts) when attempts > 0 do
    case Process.whereis(ScenarioWorker) do
      pid when is_pid(pid) and pid != old_worker ->
        pid

      _ ->
        Process.sleep(10)
        wait_for_restarted_worker(old_worker, attempts - 1)
    end
  end

  defp wait_for_restarted_worker(_old_worker, 0), do: flunk("scenario worker was not restarted")
end
