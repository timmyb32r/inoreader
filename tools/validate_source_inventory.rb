#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "uri"
require "yaml"

path = ARGV.fetch(0, "source-inventory/inventory.json")
inventory = JSON.parse(File.read(path))
errors = []
sources = inventory.fetch("sources")
expected = inventory.fetch("expected_source_count")
errors << "expected #{expected} sources, got #{sources.length}" unless sources.length == expected

ids = sources.map { |source| source["id"] }
id_counts = Hash.new(0)
ids.each { |id| id_counts[id] += 1 }
duplicates = id_counts.select { |_id, count| count > 1 }.keys
errors << "duplicate source ids: #{duplicates.join(', ')}" unless duplicates.empty?

sources.each_with_index do |entry, index|
  config = entry["configuration"]
  errors << "source #{index} id does not match preserved configuration" unless entry["id"] == config["id"]
  errors << "#{entry['id']}: missing name" unless config["name"].is_a?(String) && !config["name"].empty?
  begin
    uri = URI.parse(config.fetch("url"))
    errors << "#{entry['id']}: url must be absolute http(s)" unless %w[http https].include?(uri.scheme) && uri.host
  rescue KeyError, URI::InvalidURIError
    errors << "#{entry['id']}: invalid or missing url"
  end
  unless %w[configuration_only historical_observation_only raw_response].include?(entry["fixture_coverage"])
    errors << "#{entry['id']}: unknown fixture coverage"
  end
end

pdf = inventory.fetch("pdf_inventory")
if pdf["status"] == "complete" && pdf.fetch("rows").length != pdf.fetch("expected_rows")
  errors << "PDF marked complete with wrong row count"
end

coverage_path = File.expand_path("../source-inventory/coverage-matrix.json", __dir__)
if !File.file?(coverage_path)
  errors << "source coverage matrix is missing"
else
  coverage = JSON.parse(File.read(coverage_path))
  matrix = coverage.fetch("sources")
  errors << "coverage matrix source count differs" unless matrix.length == expected
  errors << "coverage matrix source order/identity differs" unless matrix.map { |row| row["source_id"] } == ids
  expected_kinds = sources.to_h { |source| [source["id"], source["adapter_kind"]] }
  matrix.each do |row|
    id = row["source_id"]
    errors << "#{id}: coverage adapter kind differs" unless row["adapter_kind"] == expected_kinds[id]
    errors << "#{id}: fixture evidence differs" unless row["fixture_evidence"] == sources.find { |source| source["id"] == id }&.fetch("fixture_coverage")
    errors << "#{id}: raw fixture must remain explicitly absent" unless row.key?("raw_response_fixture") && row["raw_response_fixture"].nil?
    errors << "#{id}: reviewed expectations must remain false" unless row["reviewed_expectations"] == false
    errors << "#{id}: missing explicit runtime status" unless %w[implemented_compile_only incomplete_recipe_mapping missing_specialized_adapter].include?(row["runtime_support"])
    errors << "#{id}: missing explicit coverage gap" unless row["gap"].is_a?(String) && !row["gap"].empty?
    contract = row["contract_fixture"]
    errors << "#{id}: missing evidence contract path" unless contract.is_a?(String)
    if contract.is_a?(String)
      contract_path = File.expand_path("../#{contract}", __dir__)
      if File.file?(contract_path)
        payload = JSON.parse(File.read(contract_path))
        errors << "#{id}: evidence contract identity differs" unless payload["source_id"] == id
        errors << "#{id}: evidence contract configuration differs" unless payload["configuration"] == sources.find { |source| source["id"] == id }&.fetch("configuration")
        errors << "#{id}: evidence contract must disclaim raw response provenance" unless payload.fetch("limitations", []).include?("not_a_raw_http_response")
      else
        errors << "#{id}: evidence contract is missing"
      end
    end
  end
  pdf_coverage = coverage.fetch("pdf_rows")
  errors << "coverage matrix PDF expected rows differ" unless pdf_coverage["expected"] == pdf["expected_rows"]
  errors << "coverage matrix must not claim visually reviewed PDF rows" unless pdf_coverage["exact_values_reviewed"] == pdf["rows"].length
end
if pdf["candidate_file"]
  candidate_path = File.expand_path("../#{pdf.fetch('candidate_file')}", __dir__)
  if File.file?(candidate_path)
    candidates = JSON.parse(File.read(candidate_path))
    errors << "PDF OCR expected-row count differs" unless candidates["expected_rows_from_ui"] == pdf["expected_rows"]
    candidate_rows = candidates.fetch("rows")
    errors << "PDF evidence schema must be row-aware version 2" unless candidates["schema_version"] == 2
    errors << "PDF logical-row count differs" unless candidates["detected_logical_rows"] == candidate_rows.length
    errors << "PDF geometry must account for every expected row" unless candidate_rows.length == pdf["expected_rows"]
    errors << "PDF row numbers are not contiguous" unless candidate_rows.map { |row| row["row_number"] } == (1..pdf["expected_rows"]).to_a
    errors << "PDF fragment count is inconsistent" unless candidate_rows.sum { |row| row.fetch("fragments").length } == candidates["detected_marker_fragments"]
    errors << "PDF split-row count is inconsistent" unless candidate_rows.count { |row| row.fetch("fragments").length > 1 } == candidates["split_rows_merged"]
    candidate_rows.each do |row|
      errors << "PDF row #{row['row_number']} is not explicitly unreviewed" unless row["review_status"] == "needs_visual_review"
      errors << "PDF row #{row['row_number']} lacks uncertainty" unless row.fetch("uncertainty").include?("unreviewed_ocr")
      row.fetch("fragments").each do |fragment|
        errors << "PDF row #{row['row_number']} lacks marker coordinates" unless fragment.fetch("marker_box").length == 4
        errors << "PDF row #{row['row_number']} lacks crop coordinates" unless fragment.fetch("cell_crop_box").length == 4
        errors << "PDF row #{row['row_number']} lacks raw OCR lines" unless fragment["raw_ocr_lines"].is_a?(Array)
        errors << "PDF row #{row['row_number']} lacks raw OCR words" unless fragment["raw_ocr_words"].is_a?(Array)
      end
    end
    errors << "reviewed PDF rows cannot exceed detected rows" if pdf.fetch("rows").length > candidate_rows.length
  else
    errors << "PDF OCR candidate file is missing: #{pdf['candidate_file']}"
  end
end

if ARGV[1]
  legacy = YAML.safe_load(File.read(ARGV[1]), permitted_classes: [], aliases: false).fetch("sources")
  preserved = sources.map { |source| source.fetch("configuration") }
  errors << "checked manifest differs from legacy source mappings or ordering" unless preserved == legacy
end

if errors.empty?
  covered = sources.count { |source| source["fixture_coverage"] != "configuration_only" }
  geometry_rows = if pdf["candidate_file"] && File.file?(File.expand_path("../#{pdf.fetch('candidate_file')}", __dir__))
                    JSON.parse(File.read(File.expand_path("../#{pdf.fetch('candidate_file')}", __dir__))).fetch("detected_logical_rows")
                  else
                    0
                  end
  puts "source inventory valid: #{sources.length}/#{expected} IDs; historical evidence #{covered}/#{expected}; raw response fixtures 0/#{expected}; PDF geometry #{geometry_rows}/#{pdf['expected_rows']}, reviewed #{pdf['rows'].length}/#{pdf['expected_rows']} (#{pdf['status']})"
else
  warn errors.join("\n")
  exit 1
end
