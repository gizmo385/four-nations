(ns four-nations.model.map.biomes
  "The idea for this is still under development. It would likely involve subdividing the game map
   and then assigning biomes and growing those biomes.

   For the actual implementation, potentially reference this StackOverflow post:
    https://stackoverflow.com/questions/3397972/divide-grid-2d-array-into-random-shaped-parts"
  (:require
    [four-nations.general.utils :as utils]
    [four-nations.general.types :refer [->Point]]
    [four-nations.model.map.utils :as map-utils]
    [random-seed.core :as rs]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Algorithm for randomly subdividing the map and adding IDs to it.
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn select-random-points
  [dimensions points-to-generate]
  (for [_ (range points-to-generate)]
    (->Point (int (rs/rand (:width dimensions)))
             (int (rs/rand (:height dimensions))))))

(defn fully-grouped?
  "Given a map m, determines if the tiles in that map have been fully-grouped by a group ID."
  [m]
  (->> m vals (map #(get-in % [:attributes :group-id])) (every? some?)))

(defn select-group-id-from-neighbors
  "Given a tile from a map, randomly selects one of the group IDs from that tiles neighbors."
  [tile m dimensions]
  (let [group-id (some->> (map-utils/randomly-map-over-neighbors
                            (fn [neighbor] (get-in neighbor [:attributes :group-id]))
                            tile
                            m
                            dimensions
                            false)
                          (filter some?)
                          (not-empty)
                          (rs/rand-nth))]
    (assoc-in tile [:attributes :group-id] group-id)))

(defn spread-groups
  "Given map, spreads tile ids to neighbors."
  [m dimensions]
  (map-utils/randomly-map-over-tiles
    (fn [tile]
      ;; If the tile doesn't have a group ID, then we'll set its group ID to a random selection of
      ;; one of the group IDs next to it (or skip it if none of its neighbors have group IDs).
      (if (some? (get-in tile [:attributes :group-id]))
        tile
        (select-group-id-from-neighbors tile m dimensions)))
    m
    dimensions))

(defn subdivide-map
  "Given a map and its dimensions, subdivides a map into the specified number of divisions, with each
   division ID'd from 0 to the number of subgroup IDs specified. This means you could divide the map
   into 10 different segments and each segment would be have an ID between [0, 4]."
  [m dimensions subgroup-count subgroup-id-count]
  (let [initial-points (select-random-points dimensions subgroup-count)
        initial-tiling (reduce
                         (fn [game-map [idx p]]
                           (assoc-in game-map [p :attributes :group-id] idx))
                         m
                         (zipmap (repeatedly #(int (rs/rand subgroup-id-count))) initial-points))]
    (loop [updated-map initial-tiling]
      (if (fully-grouped? updated-map)
        updated-map
        (recur (spread-groups updated-map dimensions))))))


;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Biome generation logic
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn group-ids->biomes
  "Replaces the group ID on each tile with the biome that group ID corresponds to."
  [m dimensions id->biome]
  (map-utils/map-over-tiles
    (fn [tile]
      (let [biome (-> tile (get-in [:attributes :group-id]) id->biome)]
        (-> tile
            (assoc-in [:attributes :biome] biome)
            (update :attributes dissoc :group-id))))
    m
    dimensions))

(defn add-biomes
  "Given a map and its dimensions, divides the map and adds biomes from a defined set of available
   biomes."
  [m dimensions biome-count biomes]
  (let [id->biome (zipmap (range) biomes)]
    (-> (into {}  m)
        (subdivide-map dimensions biome-count (count biomes))
        (group-ids->biomes dimensions id->biome))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Biome definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord Biome [name])

(defn load-biome-definitions
  "Given a filename with EDN biome definitions, returns Biome records"
  [filename]
  (->> filename
       utils/load-edn-resource
       (map map->Biome)))

(comment
  (require
    '[four-nations.model.map :as m]
    '[four-nations.general.types :refer [->Dimension]]
    '[clojure.pprint :refer [pprint]])
  (use 'clojure.pprint)
  (let [dimensions (->Dimension 175 30)
        group-count 10
        group-id-count 5
        game-map (m/build-map dimensions)
        grouped-map (subdivide-map game-map dimensions group-count group-id-count)]
    (doseq [x (range (:width dimensions))]
      (doseq [y (range (:height dimensions))]
        (let [group-id (-> grouped-map (map-utils/get-cell x y) (get-in [:attributes :group-id]))]
          (print group-id)))
      (println))))
