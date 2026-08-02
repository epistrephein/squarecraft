# frozen_string_literal: true

RSpec.describe Squarecraft::Config do
  before do
    # Reset cached configuration between tests
    described_class.instance_variable_set(:@configuration, nil)

    # Remove dynamically defined methods from prior runs
    %i[palettes geometries].each do |method|
      if described_class.singleton_class.method_defined?(method)
        described_class.singleton_class.remove_method(method)
      end
    end
  end

  describe ".configuration" do
    it "returns a hash" do
      expect(described_class.configuration).to be_a(Hash)
    end

    it "loads palettes from config/palettes.yml" do
      expect(described_class.configuration).to have_key(:palettes)
    end

    it "loads geometries from config/geometries.yml" do
      expect(described_class.configuration).to have_key(:geometries)
    end

    it "caches the configuration" do
      first_call = described_class.configuration
      second_call = described_class.configuration
      expect(first_call).to be(second_call)
    end
  end

  describe "dynamic accessors" do
    it "defines a method for palettes" do
      described_class.configuration
      expect(described_class.palettes).to be_a(Hash)
    end

    it "returns palette data with symbolized keys" do
      palettes = described_class.palettes
      expect(palettes).to have_key(:estuary)
      expect(palettes[:estuary]).to have_key(:background)
      expect(palettes[:estuary]).to have_key(:colors)
    end

    it "defines a method for geometries" do
      described_class.configuration
      expect(described_class.geometries).to be_a(Hash)
    end

    it "returns geometry data with symbolized keys" do
      geometries = described_class.geometries
      expect(geometries).to have_key(:classic)
      expect(geometries[:classic]).to have_key(:rows)
    end
  end

  describe ".respond_to_missing?" do
    it "returns true for loaded config keys" do
      described_class.configuration
      expect(described_class.respond_to?(:palettes)).to be(true)
    end

    it "returns false for unknown keys" do
      expect(described_class.respond_to?(:nonexistent_config)).to be(false)
    end
  end

  describe ".method_missing" do
    it "delegates to dynamic accessor for known keys" do
      result = described_class.palettes
      expect(result).to be_a(Hash)
    end

    it "raises NoMethodError for unknown methods" do
      expect { described_class.nonexistent_method }.to raise_error(NoMethodError)
    end
  end
end
