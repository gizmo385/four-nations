(ns four-nations.model.map.enrichments
  (:require
    [four-nations.model.map.biomes :as biomes]
    [four-nations.model.map.utils :as utils]
    [four-nations.model.map.resources :as res]
    [four-nations.general.types :refer [->Dimension]]
    [random-seed.core :as rs]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Helper functions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn average-map-value
  "Given a noise map, calculates the average value of all of the cells within that noise map."
  [noise]
  (let [all-values (map :value (vals noise))]
    (quot (apply + all-values)
          (count all-values))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initial map tile creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn determine-cell-terrain-type
  "Determines if a cell should be land, water, or mountain based on the cell value's relation to
   the average value of the game map."
  [cell-value average-value]
  (cond
    (> cell-value (+ average-value 20)) :mountain
    (< cell-value average-value) :water
    :else :land))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Various enrichments to the game map, such as adding coastline or placing resources
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn water-should-spread-to-tile?
  "Determines if water should spread to a particular tile, based on the terrain types of the
   neighbors and a random chance."
  [tile m dimension spread-chance]
  (and (utils/some-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimension)
       (utils/has-terrain-type? :land tile)
       (< (rs/rand) spread-chance)))

(defn spread-water
  "Given a game map and spread chance, for every land tile within the game that is near water, the
   spread chance determines how likely it is that the water will spread to that tile."
  [m dimension {:keys [water-spread-chance]}]
  (utils/map-over-tiles
    (fn [tile]
      (if (water-should-spread-to-tile? tile m dimension water-spread-chance)
        (assoc-in tile [:attributes :terrain-type] :water)
        tile))
    m
    dimension))

(defn tile-should-be-drenched?
  "Given a tile, a map, and its dimensions, determines if that tile should be drenched with water,
   which happens when a land tile is surrounded by water tiles."
  [tile m dimension]
  (and (utils/all-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimension)
       (utils/has-terrain-type? :land tile)))

(defn drench-surrounded-land
  "Given a game map, finds any land tiles where all of its cardinal neighbors are water and sets
   those tiles to also be water. This prevents tiny, nonsensical islands."
  [m dimension]
  (utils/map-over-tiles
    (fn [tile]
      (if (tile-should-be-drenched? tile m dimension)
        (assoc-in tile [:attributes :terrain-type] :water)
        tile))
    m
    dimension))

(defn should-be-coastline?
  "Returns true if the specified tile should be coastline."
  [tile m dimension]
  (and (utils/has-terrain-type? :land tile)
       (utils/some-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimension)))

(defn add-coastline
  "Given a map, changes the terrain type of any land tiles that are near water to be coastline."
  [m dimension]
  (utils/map-over-tiles
    (fn [tile]
      (if (should-be-coastline? tile m dimension)
        (assoc-in tile [:attributes :terrain-type] :coast)
        tile))
    m
    dimension))

(defn add-water-border
  "Adds a water border, where all tiles along the edge of the map become water, of a specified size
   to the map."
  [m dimension {:keys [water-border]}]
  (let [water-width-edge (- (:width dimension) water-border)
        water-height-edge (- (:height dimension) water-border)]
    (map
      (fn [[{:keys [x y] :as point} tile]]
        [point
         (if (or (< x water-border)
                 (>= x water-width-edge)
                 (< y water-border)
                 (>= y water-height-edge))
           (assoc-in tile [:attributes :terrain-type] :water)
           tile)])
      m)))

(defn maybe-add-resource-to-tile
  [tile {:keys [spawn-predicate] :as resource}]
  (if (and (nil? (get-in tile [:attributes :resource]))
           (spawn-predicate tile))
    (assoc-in tile [:attributes :resource] resource)
    tile))

(defn add-resources
  "Given a map, its dimensions and available resources; adds resources to the map based on the
   specified spawn predicates for those resources."
  [m dimension {:keys [resources]}]
  (utils/map-over-tiles
    (fn [tile]
      (reduce maybe-add-resource-to-tile tile resources))
    m
    dimension))

(defn add-basic-terrain
  "Given the noise map and the average value of cells across the map, builds basic game map with
   simple terrain selection."
  [noise average-value]
  (map (fn [[point tile]]
         (let [terrain-type (determine-cell-terrain-type (:value tile) average-value)]
           [point (assoc-in tile [:attributes :terrain-type] terrain-type)]))
       noise))

(defn add-biomes
  "Adds biomes to the map through random subdivision."
  [m dimension {:keys [biomes biome-count]}]
  (biomes/add-biomes m dimension biome-count biomes))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Main entry point for taking a noise map and adding enrichments to it
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def default-biomes (biomes/load-biome-definitions "biomes.edn"))
(def default-resources (res/load-resource-definitions "resources.edn"))
(def default-enrichment-options
  {:water-spread-chance 0.1
   :water-border 2
   :biome-count 5
   :biomes default-biomes
   :resources default-resources})

(defn enrich-noise-map
  "Given a generated and smoothed noisemap, generates a game map with additional attributes."
  ([noise dimension]
   (enrich-noise-map noise dimension default-enrichment-options))

  ([noise dimension options]
   (let [average-value (average-map-value noise)]
     (-> noise
         (add-basic-terrain average-value)
         (add-biomes dimension options)
         (add-water-border dimension options)
         (spread-water dimension options)
         (drench-surrounded-land dimension)
         (add-coastline dimension)
         (add-resources dimension options)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Rich comments for experimenting :)
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(comment
  (require
    '[four-nations.model.map.noise-map :as nm]
    '[clojure.pprint :refer [pprint]])

  (let [dimension (->Dimension 175 40)
        smoothing-passes 15
        noise (nm/generate-noisemap dimension smoothing-passes)
        enriched-map (enrich-noise-map noise dimension)]
    (utils/print-map enriched-map dimension)
    )
  )
