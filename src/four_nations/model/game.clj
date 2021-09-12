(ns four-nations.model.game
  (:require
    [four-nations.general.types :refer [->Point ->Dimension]]
    [four-nations.general.utils :as utils]
    [four-nations.model.map :as m]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.model.units.core :as units]
    [random-seed.core :as rs]
    ))

(defrecord Game [game-map map-dimensions units])

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
  [unit-count spawn-square-dimensions unit-type])

(defrecord PotentialSpawnSquare
  [top-left-point spawn-square-dimensions])

(defn can-spawn-unit-here?
  "Determines whether or not a unit can be spawned on a particular map tile. We don't spawn units in
   water and they must not spawn on top of a resource."
  [tile]
  (and (some-> tile :attributes :terrain-type (= :land))
       (-> tile :attributes :resource nil?)))

(defn spawn-square->valid-spawn-points
  "Given a spawn square on a map, the square is a valid spawn square if there are enough tiles to
   spawn units on. If the square is valid, the available spawn points will be returned, otherwise
   nil will be returned."
  [game-map unit-count spawn-square]
  (let [top-left-x (get-in spawn-square [:top-left-point :x])
        top-left-y (get-in spawn-square [:top-left-point :y])
        spawn-square-dimensions (:spawn-square-dimensions spawn-square)
        valid-tiles (->> (for [x (range top-left-x (+ top-left-x (:width spawn-square-dimensions)))
                              y (range top-left-y (+ top-left-y (:height spawn-square-dimensions)))]
                          (map-utils/get-cell game-map (->Point x y)))
                        (filter can-spawn-unit-here?))]
    (when (-> valid-tiles count (> unit-count)) valid-tiles)))

(defn random-starting-point
  "Generates a random spawn square starting point based on the map dimensions"
  [map-dimensions]
  (->Point (int (rs/rand (:width map-dimensions)))
           (int (rs/rand (:height map-dimensions)))))

(defn find-valid-spawn-points
  "Given a game map and information about how the user wants to place units, returns all valid spawn
   points from random potential spawn points."
  [game-map map-dimensions spawn-square-dimensions unit-count]
  (let [random-starting-points (repeatedly (partial random-starting-point map-dimensions))]
    (reduce
      (fn [_ starting-point]
        (some->> (->PotentialSpawnSquare starting-point spawn-square-dimensions)
                 (spawn-square->valid-spawn-points game-map unit-count)
                 (reduced)))
      random-starting-points)))

(defn spawn-units
  "Given a game, a strategy for spawning units, and a tribe, creates and spawns new units in
   a valid spawn square."
  [{:keys [game-map map-dimensions] :as game}
   {:keys [spawn-square-dimensions unit-count unit-type] :as unit-spawn-strategy}
   tribe]
  (let [valid-spawn-points (->> (find-valid-spawn-points game-map map-dimensions spawn-square-dimensions unit-count)
                                utils/seeded-shuffle
                                (take unit-count))]
    (reduce
      (fn [game spawn-point]
        (let [new-unit (units/unit-type->unit unit-type tribe)]
          (assoc-in game [:units (:point spawn-point)] new-unit)))
      game
      valid-spawn-points)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initializing the game
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord GameInitializationStrategy
  [map-generation-strategy unit-spawn-strategy tribe])

(defn initialize-game
  "Creates a game map and spawns initial units in the game."
  [{:keys [map-generation-strategy unit-spawn-strategy tribe] :as game-init-strategy}]
  (let [game-map (initialize-map map-generation-strategy)]
    (-> game-map
        (->Game (:map-dimensions map-generation-strategy) nil)
        (spawn-units unit-spawn-strategy tribe))))
