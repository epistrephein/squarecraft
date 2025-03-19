# frozen_string_literal: true

require "yaml"

module Squarecraft
  class Config
    class << self
      def configuration
        @configuration ||= load_configs!
      end

      private

      def load_configs!
        configuration = {}

        Dir.glob(File.expand_path("../config/*.yml", __dir__)).each do |file|
          key = File.basename(file, ".yml").downcase.to_sym
          configuration[key] = YAML.load_file(file, symbolize_names: true)

          define_singleton_method(key) { configuration[key] }
        end

        configuration
      end

      def method_missing(name, *args, &block)
        return send(name, *args, &block) if respond_to_missing?(name)

        super
      end

      def respond_to_missing?(name, include_private = false)
        configuration.key?(name) || super
      end
    end
  end
end
