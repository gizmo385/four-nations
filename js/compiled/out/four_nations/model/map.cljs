(ns four-nations.model.map
  (:require
    [four-nations.model.map.noise-map :as nm]
    [four-nations.model.map.enrichments :as enrichments]))

(defn build-map
  "Builds a noisemap and then enriches that noise map to form a final game map."
  [dimensions smoothing-passes enrichment-options]
  (let [final-enrichment-options (merge enrichments/default-enrichment-options enrichment-options)]
    (println final-enrichment-options)
    (-> (nm/generate-noisemap dimensions smoothing-passes)
        (enrichments/enrich-noise-map dimensions final-enrichment-options))))
