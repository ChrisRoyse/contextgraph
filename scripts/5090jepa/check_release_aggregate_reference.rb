#!/usr/bin/env ruby
# frozen_string_literal: true

require "csv"
require "digest"
require "json"
require "optparse"

TABLE_SPECS = {
  "table_prediction_metrics_multiseed" => {
    "path" => "paper_tables/table_prediction_metrics_multiseed.csv",
    "key_columns" => %w[domain metric],
    "exact_columns" => %w[domain metric n source_career_training_seeds],
    "numeric_columns" => %w[mean std bca_lower bca_upper min max],
    "volatile_columns" => %w[source_run_ids]
  },
  "table_ablation_multiseed" => {
    "path" => "paper_tables/table_ablation_multiseed.csv",
    "key_columns" => %w[ablation_id metric],
    "exact_columns" => %w[ablation_id metric n source_career_training_seeds],
    "numeric_columns" => %w[mean std bca_lower bca_upper min max],
    "volatile_columns" => %w[source_run_ids]
  },
  "table_paired_ablation_tests" => {
    "path" => "paper_tables/table_paired_ablation_tests.csv",
    "key_columns" => %w[metric baseline_ablation_id comparison_ablation_id],
    "exact_columns" => %w[
      metric baseline_ablation_id comparison_ablation_id n source_career_training_seeds
    ],
    "numeric_columns" => %w[mean_delta std_delta bca_lower bca_upper min_delta max_delta sign_flip_p],
    "volatile_columns" => %w[source_run_ids]
  },
  "table_seed_failures" => {
    "path" => "paper_tables/table_seed_failures.csv",
    "key_columns" => %w[run_id],
    "exact_columns" => %w[run_id career_training_seed status error_code error_message bundle_path],
    "numeric_columns" => [],
    "volatile_columns" => []
  }
}.freeze

EXACT_METRICS = %w[
  train_rows val_rows test_rows parameter_count panel_slots input_dim target_dim latent_dim action_dim
].freeze

DEFAULT_NUMERIC_TOLERANCE = 0.10
EXACT_NUMERIC_TOLERANCE = 1.0e-12

def die_json(operation, error_code, message, remediation, extra = {})
  puts JSON.pretty_generate({
    "operation" => operation,
    "status" => "error",
    "error_code" => error_code,
    "error_message" => message,
    "remediation" => remediation
  }.merge(extra))
  exit 1
end

def path_state(path)
  if File.exist?(path)
    {
      "path" => path,
      "exists" => true,
      "kind" => File.directory?(path) ? "directory" : "file",
      "size_bytes" => File.file?(path) ? File.size(path) : nil,
      "sha256" => File.file?(path) ? Digest::SHA256.file(path).hexdigest : nil
    }
  else
    { "path" => path, "exists" => false }
  end
end

def read_json(path, field)
  JSON.parse(File.read(path))
rescue Errno::ENOENT => e
  raise "#{field} missing: #{e.message}"
rescue JSON::ParserError => e
  raise "#{field} is invalid JSON: #{e.message}"
end

def read_table(path, allow_empty: false)
  rows = CSV.read(path, headers: true).map(&:to_h)
  if CSV.read(path, headers: true).headers.nil?
    raise "CSV has no header: #{path}"
  end
  raise "CSV has no data rows: #{path}" if rows.empty? && !allow_empty

  {
    "headers" => CSV.read(path, headers: true).headers,
    "rows" => rows
  }
rescue Errno::ENOENT => e
  raise "CSV missing: #{e.message}"
rescue CSV::MalformedCSVError => e
  raise "CSV malformed: #{path}: #{e.message}"
end

def table_key(row, columns)
  columns.map { |column| row.fetch(column) }.join("\u001f")
end

def numeric_tolerance(row, column)
  return EXACT_NUMERIC_TOLERANCE if EXACT_METRICS.include?(row["metric"])
  return 0.15 if column == "sign_flip_p"

  DEFAULT_NUMERIC_TOLERANCE
end

def normalize_table_for_reference(aggregate_root, table_name, spec)
  table = read_table(File.join(aggregate_root, spec["path"]), allow_empty: table_name == "table_seed_failures")
  rows = table["rows"].map do |row|
    row.reject { |key, _| spec["volatile_columns"].include?(key) }
  end
  {
    "path" => spec["path"],
    "key_columns" => spec["key_columns"],
    "headers" => table["headers"],
    "volatile_columns" => spec["volatile_columns"],
    "rows" => rows.sort_by { |row| table_key(row, spec["key_columns"]) }
  }
end

def build_pairwise_mi_audit_references(aggregate_root)
  Dir.glob(File.join(aggregate_root, "pairwise_mi_audit_*")).sort.filter_map do |audit_dir|
    next unless File.directory?(audit_dir)

    audit_path = File.join(audit_dir, "pairwise_mi_audit.json")
    summary_path = File.join(audit_dir, "paper_tables/table_pairwise_mi_summary.csv")
    matrix_path = File.join(audit_dir, "pairwise_mi_matrix.csv")
    heatmap_path = File.join(audit_dir, "pairwise_mi_heatmap.svg")
    [audit_path, summary_path, matrix_path, heatmap_path].each do |path|
      state = path_state(path)
      unless state["exists"] && state["kind"] == "file" && state["size_bytes"].to_i.positive?
        raise "pairwise MI audit output is missing or empty: #{path}"
      end
    end

    audit = read_json(audit_path, "pairwise_mi_audit")
    summary = audit.fetch("summary")
    domain = summary.fetch("domain")
    n_panel_slots = summary.fetch("n_panel_slots").to_i
    {
      "domain" => domain,
      "path" => File.basename(audit_dir),
      "summary_path" => File.join(File.basename(audit_dir), "paper_tables/table_pairwise_mi_summary.csv"),
      "matrix_path" => File.join(File.basename(audit_dir), "pairwise_mi_matrix.csv"),
      "heatmap_path" => File.join(File.basename(audit_dir), "pairwise_mi_heatmap.svg"),
      "audit_json_path" => File.join(File.basename(audit_dir), "pairwise_mi_audit.json"),
      "expected" => {
        "sample_size" => audit.fetch("estimator").fetch("sample_size").to_i,
        "n_panel_slots" => n_panel_slots,
        "n_pairs" => summary.fetch("n_pairs").to_i,
        "n_eff_loose_min" => 9.0,
        "n_eff_loose_max" => n_panel_slots.to_f,
        "n_eff_strict_min" => 9.0,
        "n_eff_strict_max" => n_panel_slots.to_f
      }
    }
  end
end

def build_reference(aggregate_root, reference_id)
  manifest = read_json(File.join(aggregate_root, "aggregate_manifest.json"), "aggregate_manifest")
  fsv = read_json(File.join(aggregate_root, "aggregate_fsv_report.json"), "aggregate_fsv_report")
  pairwise_mi_audits = build_pairwise_mi_audit_references(aggregate_root)

  reference = {
    "schema_version" => 1,
    "kind" => "dynamicjepa_release_aggregate_reference",
    "reference_id" => reference_id,
    "source_aggregate" => File.expand_path(aggregate_root),
    "expected" => {
      "status" => "passed",
      "passed_count" => manifest["passed_count"],
      "failed_count" => manifest["failed_count"],
      "required_career_training_seeds" => manifest.dig("selection", "required_career_training_seeds"),
      "expected_bundle_count" => manifest.dig("selection", "expected_bundle_count"),
      "min_passed" => manifest.dig("selection", "min_passed"),
      "expected_purpose" => manifest.dig("selection", "expected_purpose"),
      "expected_career_event_count" => manifest.dig("selection", "expected_career_event_count"),
      "bootstrap" => manifest["bootstrap"],
      "readback_checks" => fsv["readback_checks"]
    },
    "comparison_policy" => {
      "default_numeric_tolerance" => DEFAULT_NUMERIC_TOLERANCE,
      "exact_numeric_tolerance" => EXACT_NUMERIC_TOLERANCE,
      "exact_metrics" => EXACT_METRICS,
      "ignored_columns" => TABLE_SPECS.transform_values { |spec| spec["volatile_columns"] }
    },
    "required_file_states" => fsv["required_file_states"],
    "tables" => TABLE_SPECS.to_h do |table_name, spec|
      [table_name, normalize_table_for_reference(aggregate_root, table_name, spec)]
    end
  }
  reference["pairwise_mi_audits"] = pairwise_mi_audits unless pairwise_mi_audits.empty?
  reference
end

def compare_table(table_name, spec, actual_root, reference_table)
  actual = normalize_table_for_reference(actual_root, table_name, spec)
  diffs = []

  if actual["headers"] != reference_table["headers"]
    diffs << {
      "type" => "header_mismatch",
      "table" => table_name,
      "actual" => actual["headers"],
      "reference" => reference_table["headers"]
    }
  end

  actual_by_key = actual["rows"].to_h { |row| [table_key(row, spec["key_columns"]), row] }
  reference_by_key = reference_table["rows"].to_h { |row| [table_key(row, spec["key_columns"]), row] }
  missing = reference_by_key.keys - actual_by_key.keys
  extra = actual_by_key.keys - reference_by_key.keys
  missing.each { |key| diffs << { "type" => "missing_row", "table" => table_name, "key" => key } }
  extra.each { |key| diffs << { "type" => "extra_row", "table" => table_name, "key" => key } }

  (actual_by_key.keys & reference_by_key.keys).each do |key|
    actual_row = actual_by_key.fetch(key)
    ref_row = reference_by_key.fetch(key)
    spec["exact_columns"].each do |column|
      next if spec["volatile_columns"].include?(column)

      if actual_row[column] != ref_row[column]
        diffs << {
          "type" => "exact_column_mismatch",
          "table" => table_name,
          "key" => key,
          "column" => column,
          "actual" => actual_row[column],
          "reference" => ref_row[column]
        }
      end
    end
    spec["numeric_columns"].each do |column|
      actual_value = Float(actual_row.fetch(column))
      ref_value = Float(ref_row.fetch(column))
      tolerance = numeric_tolerance(ref_row, column)
      delta = (actual_value - ref_value).abs
      next if delta <= tolerance

      diffs << {
        "type" => "numeric_out_of_tolerance",
        "table" => table_name,
        "key" => key,
        "column" => column,
        "actual" => actual_value,
        "reference" => ref_value,
        "abs_delta" => delta,
        "tolerance" => tolerance
      }
    rescue ArgumentError => e
      diffs << {
        "type" => "numeric_parse_error",
        "table" => table_name,
        "key" => key,
        "column" => column,
        "message" => e.message
      }
    end
  end

  {
    "table" => table_name,
    "actual_row_count" => actual["rows"].length,
    "reference_row_count" => reference_table["rows"].length,
    "diffs" => diffs
  }
end

options = {
  "mode" => "verify",
  "json" => false,
  "reference_id" => "release_v1.0.0_paper_small_1k_multiseed"
}

parser = OptionParser.new do |opts|
  opts.banner = "Usage: check_release_aggregate_reference.rb --aggregate PATH (--reference PATH | --capture --output PATH) [--json]"
  opts.on("--aggregate PATH") { |value| options["aggregate"] = value }
  opts.on("--reference PATH") { |value| options["reference"] = value }
  opts.on("--capture") { options["mode"] = "capture" }
  opts.on("--output PATH") { |value| options["output"] = value }
  opts.on("--reference-id ID") { |value| options["reference_id"] = value }
  opts.on("--json") { options["json"] = true }
end
parser.parse!(ARGV)

operation = options["mode"] == "capture" ? "dynamicjepa_capture_release_aggregate_reference" : "dynamicjepa_check_release_aggregate_reference"

aggregate = options["aggregate"]
die_json(operation, "VALIDATION", "--aggregate is required", "pass the immutable multi-seed aggregate bundle path") unless aggregate
die_json(operation, "VALIDATION", "aggregate path does not exist: #{aggregate}", "run dynamicjepa multiseed-aggregate before checking release reference") unless File.directory?(aggregate)

if options["mode"] == "capture"
  output = options["output"]
  die_json(operation, "VALIDATION", "--output is required with --capture", "provide the checked-in reference JSON path") unless output
  reference = build_reference(aggregate, options["reference_id"])
  File.write(output, JSON.pretty_generate(reference) + "\n")
  puts JSON.pretty_generate({
    "operation" => operation,
    "status" => "ok",
    "source_of_truth" => {
      "aggregate" => aggregate,
      "reference" => output
    },
    "after_state" => {
      "reference" => path_state(output)
    },
    "reference" => {
      "reference_id" => reference["reference_id"],
      "kind" => reference["kind"],
      "table_count" => reference["tables"].length
    }
  })
  exit 0
end

reference_path = options["reference"]
die_json(operation, "VALIDATION", "--reference is required", "provide the checked-in release aggregate reference JSON") unless reference_path
die_json(operation, "VALIDATION", "reference path does not exist: #{reference_path}", "capture or restore reference/reference_release_v1.0.0.json") unless File.file?(reference_path)

before_state = {
  "aggregate" => path_state(aggregate),
  "reference" => path_state(reference_path)
}

reference = read_json(reference_path, "reference")
unless reference["kind"] == "dynamicjepa_release_aggregate_reference"
  die_json(
    operation,
    "SCHEMA",
    "reference kind is #{reference["kind"].inspect}, expected dynamicjepa_release_aggregate_reference",
    "regenerate the release reference from a passed multi-seed aggregate with --capture",
    "before_state" => before_state
  )
end

manifest = read_json(File.join(aggregate, "aggregate_manifest.json"), "aggregate_manifest")
fsv = read_json(File.join(aggregate, "aggregate_fsv_report.json"), "aggregate_fsv_report")
expected = reference.fetch("expected")
diffs = []

{
  "manifest.status" => [manifest["status"], expected["status"]],
  "manifest.passed_count" => [manifest["passed_count"], expected["passed_count"]],
  "manifest.failed_count" => [manifest["failed_count"], expected["failed_count"]],
  "manifest.selection.expected_bundle_count" => [manifest.dig("selection", "expected_bundle_count"), expected["expected_bundle_count"]],
  "manifest.selection.min_passed" => [manifest.dig("selection", "min_passed"), expected["min_passed"]],
  "manifest.selection.expected_purpose" => [manifest.dig("selection", "expected_purpose"), expected["expected_purpose"]],
  "manifest.selection.expected_career_event_count" => [manifest.dig("selection", "expected_career_event_count"), expected["expected_career_event_count"]],
  "manifest.selection.required_career_training_seeds" => [manifest.dig("selection", "required_career_training_seeds"), expected["required_career_training_seeds"]],
  "fsv.status" => [fsv["status"], expected["status"]],
  "fsv.readback_checks" => [fsv["readback_checks"], expected["readback_checks"]]
}.each do |field, (actual, ref)|
  diffs << { "type" => "field_mismatch", "field" => field, "actual" => actual, "reference" => ref } unless actual == ref
end

file_states = TABLE_SPECS.values.map { |spec| spec["path"] } + [
  "aggregate_manifest.json",
  "aggregate_fsv_report.json",
  "plots/multiseed_val_mse_box.svg",
  "plots/multiseed_paired_delta_forest.svg",
  "plots/plot_manifest.json"
]
file_states.each do |relative|
  state = path_state(File.join(aggregate, relative))
  if !state["exists"] || state["kind"] != "file" || state["size_bytes"].to_i <= 0
    diffs << { "type" => "required_file_missing_or_empty", "path" => relative, "state" => state }
  end
end

table_results = TABLE_SPECS.map do |table_name, spec|
  result = compare_table(table_name, spec, aggregate, reference.fetch("tables").fetch(table_name))
  diffs.concat(result["diffs"])
  result.reject { |key, _| key == "diffs" }
end

status = diffs.empty? ? "ok" : "error"
output = {
  "operation" => operation,
  "status" => status,
  "source_of_truth" => {
    "aggregate" => aggregate,
    "reference" => reference_path,
    "aggregate_manifest" => File.join(aggregate, "aggregate_manifest.json"),
    "aggregate_fsv_report" => File.join(aggregate, "aggregate_fsv_report.json"),
    "paper_tables" => File.join(aggregate, "paper_tables")
  },
  "before_state" => before_state,
  "comparison" => {
    "table_results" => table_results,
    "diff_count" => diffs.length,
    "diffs" => diffs
  },
  "readback" => {
    "manifest_status" => manifest["status"],
    "fsv_status" => fsv["status"],
    "passed_count" => manifest["passed_count"],
    "failed_count" => manifest["failed_count"],
    "readback_checks" => fsv["readback_checks"]
  }
}
if diffs.empty?
  puts JSON.pretty_generate(output)
  exit 0
end

output["error_code"] = "RELEASE_REFERENCE_MISMATCH"
output["error_message"] = "release aggregate differs from checked-in aggregate reference"
output["remediation"] = "inspect comparison.diffs; fix the aggregate root cause or intentionally capture a new reference from a preserved passed aggregate"
puts JSON.pretty_generate(output)
exit 1
