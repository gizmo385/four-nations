(ns four-nations.model.map
  (:require
    [clojure.tools.cli :as cli]
    [clojure.string :as string]
    [four-nations.model.map.noise-map :as nm]
    [four-nations.model.map.game-map :as gm]))

(defn build-map
  [height width & {:keys [water-spread-chance water-border smoothing-passes]
                   :or {water-spread-chance 0.1
                        water-border 2
                        smoothing-passes 15}}]
  (-> (nm/generate-noisemap height width smoothing-passes water-border)
      (gm/noise-map->game-map water-spread-chance)))

(def cli-options
  [["-h" "--height HEIGHT" "Map height"
    :parse-fn #(Integer/parseInt %)]
   ["-w" "--width WIDTH" "Map width"
    :parse-fn #(Integer/parseInt %)]
   [nil "--smoothing-passes PASSES" "Number of times to smooth randomly generated terrain out."
    :parse-fn #(Integer/parseInt %)
    :validate [#(nat-int? %) "Should be a non-negative integer"]]
   [nil "--water-border BORDER" "The number of tiles along the edge that should be filled with water."
    :parse-fn #(Integer/parseInt %)
    :validate [#(nat-int? %) "Should be a non-negative integer"]]
   [nil "--water-spread-chance CHANCE" "The chance that water will spread to surrounding tiles."
    :parse-fn #(Double/parseDouble %)
    :validate [#(< 0 % 1) "Should be a decimal between 0 and 1"]]
   ["-c" "--[no-]color" "Print in color" :default true]

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
    (when (or (:help options)
              (nil? (:height options))
              (nil? (:width options)))
      (println (usage summary))
      (System/exit 1))
    (-> (build-map (:height options) (:width options) options)
        :game-map (gm/print-map true))))
