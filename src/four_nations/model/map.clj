(ns four-nations.model.map
  (:require
    [clojure.tools.cli :as cli]
    [clojure.string :as string]
    [four-nations.general.types :refer [->Dimension]]
    [four-nations.model.map.noise-map :as nm]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.model.map.enrichments :as enrichments]
    [random-seed.core :as rs]))

(defn build-map
  "Builds a noisemap and then enriches that noise map to form a final game map."
  [dimensions smoothing-passes enrichment-options]
  (let [final-enrichment-options (merge enrichments/default-enrichment-options enrichment-options)]
    (-> (nm/generate-noisemap dimensions smoothing-passes)
        (enrichments/enrich-noise-map dimensions final-enrichment-options))))

(def cli-options
  [["-h" "--height HEIGHT" "Map height"
    :parse-fn #(Integer/parseInt %)]
   ["-w" "--width WIDTH" "Map width"
    :parse-fn #(Integer/parseInt %)]
   [nil "--smoothing-passes PASSES" "Number of times to smooth randomly generated terrain out."
    :parse-fn #(Integer/parseInt %)
    :validate [#(nat-int? %) "Should be a non-negative integer"]
    :default 20]
   [nil "--water-border BORDER" "The number of tiles along the edge that should be filled with water."
    :parse-fn #(Integer/parseInt %)
    :validate [#(nat-int? %) "Should be a non-negative integer"]]
   [nil "--water-spread-chance CHANCE" "The chance that water will spread to surrounding tiles."
    :parse-fn #(Double/parseDouble %)
    :validate [#(< 0 % 1) "Should be a decimal between 0 and 1"]]
   [nil "--seed SEED" "Seed the random number generator"
    :parse-fn #(Integer/parseInt %)]

   [nil "--help"]])

(defn usage [options-summary]
  (->> ["Procedurally generating maps for game testing."
        ""
        "Options:"
        options-summary]
       (string/join \newline)))

(defn -main
  [& args]
  (let [{:keys [options arguments summary errors]} (cli/parse-opts args cli-options)]
    (when (:help options)
      (println (usage summary))
      (System/exit 0))

    (when (or errors
              (nil? (:height options))
              (nil? (:width options)))
      (println (string/join \newline errors))
      (println (usage summary))
      (System/exit 1))

    (let [seed (or (:seed options) (System/currentTimeMillis))]
      (println (format "Setting random seed to: %s" seed))
      (rs/set-random-seed! seed))

    (let [dimension (->Dimension (:height options) (:width options))
          smoothing-passes (:smoothing-passes options)]
      (-> (build-map dimension smoothing-passes options)
          (map-utils/print-map dimension))))
  (shutdown-agents))
