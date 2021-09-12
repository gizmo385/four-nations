(ns four-nations.model.game
  (:require
    [four-nations.general.types :refer [->Point ->Dimension]]
    [four-nations.general.utils :as utils]
    [four-nations.model.map :as m]
    [four-nations.model.map.utils :as map-utils]
    [random-seed.core :as rs]
    ))

(defrecord Civilization [game-map map-dimensions units])

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initializing the game map initialization
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord MapGenerationStrategy
  [map-dimensions smoothing-passes enrichment-options])

(defn initialize-map
  [{:keys [map-dimensions smoothing-passes enrichment-options] :as strategy}]
  (m/build-map map-dimensions smoothing-passes enrichment-options))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Spawning units on the map
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord UnitSpawnStrategy
  [unit-count spawn-square-dimensions])

(defrecord PotentialSpawnSquare
  [top-left-point spawn-square-dimensions])

(defn spawn-square->valid-spawn-points
  "Given a spawn square on a map, the square is a valid spawn square if there are enough tiles to
   spawn units on. If the square is valid, the available spawn points will be returned, otherwise
   nil will be returned."
  [game-map unit-count spawn-square]
  (let [top-left-x (get-in spawn-square [:top-left-point :x])
        top-left-y (get-in spawn-square [:top-left-point :y])
        spawn-square-dimensions (:spawn-square-dimensions spawn-square)
        land-tiles (->> (for [x (range top-left-x (+ top-left-x (:width spawn-square-dimensions)))
                              y (range top-left-y (+ top-left-y (:height spawn-square-dimensions)))]
                          (map-utils/get-cell game-map (->Point x y)))
                        (map :attributes)
                        (map :terrain-type)
                        (filter (partial = :land)))]
    (when (-> land-tiles count (> unit-count)) land-tiles)))

(defn random-starting-point
  "Generates a random spawn square starting point based on the map dimensions"
  [map-dimensions]
  (->Point (int (rs/rand (:width map-dimensions)))
           (int (rs/rand (:height map-dimensions)))))

(defn find-valid-spawn-points
  [game-map map-dimensions spawn-square-dimensions unit-count]
  (let [random-starting-points (repeatedly (partial random-starting-point map-dimensions))]
    (reduce
      (fn [_ starting-point]
        (some->> (->PotentialSpawnSquare starting-point spawn-square-dimensions)
                 (spawn-square->valid-spawn-points game-map unit-count)
                 (reduced))
        random-starting-points))))

(defn spawn-units
  [{:keys [game-map map-dimensions] :as civilization}
   {:keys [spawn-square-dimensions unit-count] :as unit-spawn-strategy}]
  (let [valid-spawn-points (find-valid-spawn-points
                             game-map map-dimensions spawn-square-dimensions unit-count)]
    (reduce
      (fn [civilization spawn-point]
        ;; TODO: Place the unit in the spawn point
        )
      civilization)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initializing the game
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord GameInitializationStrategy
  [map-generation-strategy unit-spawn-strategy])

(defn initialize-game
  [strategy])

(comment
  (let [dimensions (->Dimension 50 50)
        gen-strat (->MapGenerationStrategy dimensions 5 nil)
        game-map (initialize-map gen-strat)
        top-left-x 5
        top-left-y 5
        spawn-square-dimensions (->Dimension 5 5)]
    (valid-spawn-square? game-map (->SpawnSquare (->Point 10 10) spawn-square-dimensions) 5)
    )
  )
