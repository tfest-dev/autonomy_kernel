defmodule AutonomyKernelBeam.Messages.RunScenarioRequest do
  @moduledoc """
  Deterministic request for a known scenario run through the Rust CLI boundary.

  The request ID is supplied by the caller. The BEAM adapter does not generate
  request IDs.
  """

  @enforce_keys [:scenario_name, :request_id]
  defstruct [:scenario_name, :request_id]
end

defmodule AutonomyKernelBeam.Messages.RunScenarioResponse do
  @moduledoc """
  Deterministic response from the optional BEAM-to-Rust CLI boundary.
  """

  @enforce_keys [:request_id, :status, :scenario_name]
  defstruct [
    :request_id,
    :status,
    :scenario_name,
    :replay_verified,
    :events,
    :final_tick,
    :objective_satisfied,
    :raw_output,
    :error,
    :note
  ]
end

defmodule AutonomyKernelBeam.Messages.ExportArtifactRequest do
  @moduledoc """
  Deterministic request for a known scenario causal artifact.
  """

  @enforce_keys [:scenario_name, :request_id, :format]
  defstruct [:scenario_name, :request_id, :format]
end

defmodule AutonomyKernelBeam.Messages.ExportArtifactResponse do
  @moduledoc """
  Deterministic response for a causal artifact exported by the Rust CLI.
  """

  @enforce_keys [:request_id, :status, :scenario_name, :format]
  defstruct [:request_id, :status, :scenario_name, :format, :artifact, :error]
end
