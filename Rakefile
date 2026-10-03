# frozen_string_literal: true

require "bundler/setup"

require "rb_sys/extensiontask"
require "rspec/core/rake_task"
require "rubocop/rake_task"

RbSys::ExtensionTask.new("squarecraft") do |ext|
  ext.lib_dir = "lib/squarecraft"
end

RSpec::Core::RakeTask.new(:spec)
RuboCop::RakeTask.new(:rubocop)

desc "Run the Rust unit tests"
task :cargo_test do
  sh "cargo test --quiet"
end

task spec: :compile
task test: [:spec, :cargo_test, :rubocop]
task default: :test
