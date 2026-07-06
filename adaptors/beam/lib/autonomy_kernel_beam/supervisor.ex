defmodule AutonomyKernelBeam.Supervisor do
  @moduledoc """
  OTP supervisor for the optional BEAM adapter experiment.
  """

  use Supervisor

  alias AutonomyKernelBeam.ScenarioWorker

  def start_link(opts \\ []) do
    Supervisor.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(_opts) do
    children = [
      {ScenarioWorker, name: ScenarioWorker}
    ]

    Supervisor.init(children, strategy: :one_for_one)
  end
end
