#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "yaml"
require "time"

abort "usage: import_personal_feed.rb CONFIG OUTPUT [JSONL ...]" unless ARGV.length >= 2

config_path, output_path, *report_paths = ARGV
config = YAML.safe_load(File.read(config_path), permitted_classes: [], aliases: false)
sources = config.fetch("sources")

observations = Hash.new { |hash, key| hash[key] = [] }
report_paths.sort.each do |path|
  File.foreach(path).with_index(1) do |line, line_number|
    next if line.strip.empty?

    begin
      row = JSON.parse(line)
    rescue JSON::ParserError => error
      warn "#{path}:#{line_number}: #{error.message}"
      next
    end
    id = row["id"]
    next unless id.is_a?(String)

    observation = row.slice("count", "first_title", "first_url", "warning", "error")
    observation["evidence_file"] = File.basename(path)
    observations[id] << observation
  end
end

inventory = {
  "schema_version" => 1,
  "expected_source_count" => 42,
  "origin" => {
    "kind" => "personal_feed_config",
    "path_hint" => "personal_feed/config.yaml",
    "note" => "All source mappings are preserved verbatim after YAML decoding; ordering matches the source file."
  },
  "sources" => sources.map do |source|
    id = source.fetch("id")
    {
      "id" => id,
      "configuration" => source,
      "adapter_kind" => source.fetch("adapter", source["feed_url"] ? "standard_feed" : "generic_html"),
      "historical_observations" => observations.fetch(id, []),
      "fixture_coverage" => observations[id].empty? ? "configuration_only" : "historical_observation_only"
    }
  end,
  "pdf_inventory" => {
    "expected_rows" => 214,
    "status" => "blocked_missing_input",
    "rows" => [],
    "note" => "The nine-page raster PDF referred to as 00_29_30 in SPEC.md was not present at the documented personal_feed location or discoverable by that name. No rows are fabricated."
  }
}

File.write(output_path, JSON.pretty_generate(inventory) + "\n")
