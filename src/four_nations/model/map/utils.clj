(ns four-nations.model.map.utils
  (:require
    [clojure.term.colors :as color]
    [four-nations.general.types :refer [->Dimension ->Point]]))

(defn get-cell
  "Given a 2 dimensional game map and an (x, y) coordinate pair, retrieves the value at the
   specified cell. If an invalid x/y pair are passed in, this function will return nil."
  ([m p] (get m p))
  ([m x y] (get m (->Point x y))))

(defn valid-coordinate?
  "A coordinate is valid if both components are non-negative integers and they are both less than
   their respective bounds (y < height, x < width). Assumes zero-indexed coordinates."
  [{:keys [height width] :as dimension} {:keys [x y] :as point}]
  (and (nat-int? x) (nat-int? y) (< y height) (< x width)))

(defn point->neighbor-points
  "Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
   neighbors. Valid coordinates are checked against the valid-coordinate? predicate"
  ([{:keys [height width] :as dimension} {:keys [x y] :as point}]
   (point->neighbor-points dimension point true))

  ([{:keys [height width] :as dimension} {:keys [x y] :as point} include-diagonal?]
   (let [cardinal-neighbors [(->Point x (inc y))
                             (->Point x (dec y))
                             (->Point (inc x) y)
                             (->Point (dec x) y)]
         diagonal-neighbors [(->Point (inc x) (inc y))
                             (->Point (inc x) (dec y))
                             (->Point (dec x) (inc y))
                             (->Point (dec x) (dec y))]]
     (->> cardinal-neighbors
          (concat (when include-diagonal? diagonal-neighbors))
          (filter (partial valid-coordinate? dimension))
          set))))

(def
  ^{:arglists '([dimension point]
                [dimension point include-diagonal?])}
  point->neighbor-points
  "Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
   neighbors. Valid coordinates are checked against the valid-coordinate? predicate"
  (memoize point->neighbor-points))

(defn point->neighbors
  "Given a height, a width, a map m, and a (x, y) coordinate pair, returns the values of all valid
   neighbors, as defined by point->neighbor-coordinates."
  ([dimension m point]
   (point->neighbors dimension m point true))

  ([dimension m point include-diagonal?]
   (->> (point->neighbor-points dimension point include-diagonal?)
        (map (partial get-cell m)))))

(defn map-over-tiles
  [f m dimension]
  (->> m
       (map (fn [[point tile]] [point (f tile)]))
       (into {})))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Checking conditions on tiles and neighbors
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn map-over-neighbors
  "Applies f to every neighbor of the tile."
  [f tile m dimensions]
  (let [neighbors (point->neighbors dimensions m (:point tile))]
    (map f neighbors)))

(defn all-neighbors-are?
  "Returns true if every neighbor of the tile satisfies the predicate."
  [pred tile m dimensions]
  (every? true? (map-over-neighbors pred tile m dimensions)))

(defn some-neighbors-are?
  "Returns true if at least one neighbor of the tile satisfy the predicate."
  [pred tile m dimensions]
  (boolean (some true? (map-over-neighbors pred tile m dimensions))))

(defn no-neighbors-are?
  "Returns true if no neighbors of the tile satisfy the predicate."
  [pred tile m dimensions]
  (not (some-neighbors-are? pred tile m dimensions)))

(defn has-terrain-type?
  "Returns true if the tile has the specified terrain type."
  [terrain-type tile]
  (-> tile (get-in [:attributes :terrain-type]) (= terrain-type)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Map subtiling
;;;
;;; Subdividing the map into smaller tiles
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;


;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Printing maps to stdout
;;;
;;; This will ideally be replaced later with a more functional UI, but it is the best way to test
;;; at the moment.
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def terrain-type->color
  {:land color/green
   :water color/blue
   :mountain color/bold
   :coast color/yellow})

(def terrain-type->char
  {:land "*"
   :water "~"
   :mountain "^"
   :coast "."})

(defn- tile->printable-char
  [tile color?]
  (let [terrain-type (get-in tile [:attributes :terrain-type])
        resource (get-in tile [:attributes :resource])
        color (or (when color? (terrain-type->color terrain-type)) identity)
        output-char (or (:symbol resource) (terrain-type->char terrain-type) "?")]
    (color output-char)))

(defn print-map
  "Given a game map, prints it out in a readable way."
  ([m dimension]
   (print-map m dimension false))

  ([m dimension color?]
   (doseq [x (range (:width dimension))]
     (doseq [y (range (:height dimension))]
       (let [tile (get-cell m (->Point x y))]
         (print (tile->printable-char tile color?))))
     (println))))

