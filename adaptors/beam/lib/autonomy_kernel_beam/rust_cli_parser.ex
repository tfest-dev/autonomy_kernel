defmodule AutonomyKernelBeam.RustCliParser do
  @moduledoc """
  Strict parser for deterministic `autonomy-cli run-scenario` output.

  The parser accepts only the fixed key/value shape emitted by the Rust CLI.
  It does not infer missing values or parse arbitrary text.
  """

  @required_keys [
    "scenario",
    "status",
    "replay_verified",
    "events",
    "final_tick",
    "objective_satisfied"
  ]

  @known_keys MapSet.new(@required_keys)

  def parse_run_scenario_output(output) when is_binary(output) do
    output
    |> String.split("\n", trim: true)
    |> parse_lines(%{})
    |> case do
      {:ok, fields} -> build_summary(fields, output)
      error -> error
    end
  end

  defp parse_lines([], fields), do: {:ok, fields}

  defp parse_lines([line | rest], fields) do
    case String.split(line, ": ", parts: 2) do
      [key, value] ->
        cond do
          not MapSet.member?(@known_keys, key) ->
            {:error, {:unknown_key, key}}

          Map.has_key?(fields, key) ->
            {:error, {:duplicate_key, key}}

          true ->
            parse_lines(rest, Map.put(fields, key, value))
        end

      _ ->
        {:error, {:malformed_line, line}}
    end
  end

  defp build_summary(fields, raw_output) do
    with :ok <- require_keys(fields),
         {:ok, :ok} <- parse_status(fields["status"]),
         {:ok, replay_verified} <- parse_bool("replay_verified", fields["replay_verified"]),
         {:ok, events} <- parse_non_negative_integer("events", fields["events"]),
         {:ok, final_tick} <- parse_non_negative_integer("final_tick", fields["final_tick"]),
         {:ok, objective_satisfied} <-
           parse_bool("objective_satisfied", fields["objective_satisfied"]) do
      {:ok,
       %{
         scenario_name: fields["scenario"],
         status: :ok,
         replay_verified: replay_verified,
         events: events,
         final_tick: final_tick,
         objective_satisfied: objective_satisfied,
         raw_output: raw_output
       }}
    end
  end

  defp require_keys(fields) do
    case Enum.find(@required_keys, &(not Map.has_key?(fields, &1))) do
      nil -> :ok
      key -> {:error, {:missing_key, key}}
    end
  end

  defp parse_status("ok"), do: {:ok, :ok}
  defp parse_status(value), do: {:error, {:invalid_status, value}}

  defp parse_bool(_key, "true"), do: {:ok, true}
  defp parse_bool(_key, "false"), do: {:ok, false}
  defp parse_bool(key, value), do: {:error, {:invalid_boolean, key, value}}

  defp parse_non_negative_integer(key, value) do
    case Integer.parse(value) do
      {number, ""} when number >= 0 -> {:ok, number}
      _ -> {:error, {:invalid_integer, key, value}}
    end
  end
end
