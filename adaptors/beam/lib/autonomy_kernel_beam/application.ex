defmodule AutonomyKernelBeam.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    AutonomyKernelBeam.Supervisor.start_link([])
  end
end
