(ns four-nations.core
  (:require
    [four-nations.model.map.noise-map :as nm]
    [four-nations.model.map.game-map :as gm])
  (:gen-class))

(defn -main
  "Starts the game"
  [& args]
  (let [height 60
        width 275
        water-spread-chance 0.1
        water-border 2
        smoothing-passes 15]
    (-> (nm/generate-noisemap height width smoothing-passes water-border)
        (gm/noise-map->game-map water-spread-chance)
        :game-map
        (gm/print-map true))))
