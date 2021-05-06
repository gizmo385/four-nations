(ns four-nations.model.map.utils
  (:require
    [com.climate.claypoole :as cp]
    [clojure.term.colors :as color]
    [clojure.core.matrix :as mat]))

(defn get-cell
  "Given a 2 dimensional game map and an (x, y) coordinate pair, retrieves the value at the
   specified cell. If an invalid x/y pair are passed in, this function will return nil."
  [m x y]
  (try (mat/mget m y x)
       (catch IndexOutOfBoundsException e nil)))

(defn valid-coordinate?
  "A coordinate is valid if both components are non-negative integers and they are both less than
   their arespective bounds (y < height, x < width). Assumes zero-indexed coordinates."
  [height width [x y]]
  (and (nat-int? x) (nat-int? y) (< y height) (< x width)))

(defn coordinates->neighbor-coordinates*
  "Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
   neighbors. Valid coordinates are checked against the valid-coordinate? predicate"
  ([height width [x y]]
   (coordinates->neighbor-coordinates* height width [x y] true))

  ([height width [x y] include-diagonal?]
   (let [cardinal-neighbors [[x (inc y)]
                             [x (dec y)]
                             [(inc x) y]
                             [(dec x) y]]
         diagonal-neighbors [[(inc x) (inc y)]
                             [(inc x) (dec y)]
                             [(dec x) (inc y)]
                             [(dec x) (dec y)]]]
     (->> cardinal-neighbors
          (concat (when include-diagonal? diagonal-neighbors))
          (filter (partial valid-coordinate? height width))
          set))))

(def
  ^{:arglists '([height width [x y]]
                [height width [x y] include-diagonal?])}
  coordinates->neighbor-coordinates
  "Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
   neighbors. Valid coordinates are checked against the valid-coordinate? predicate"
  (memoize coordinates->neighbor-coordinates*))

(defn coordinates->neighbors
  "Given a height, a width, a map m, and a (x, y) coordinate pair, returns the values of all valid
   neighbors, as defined by coordinates->neighbor-coordinates."
  ([height width m [x y]]
   (coordinates->neighbors height width m [x y] true))

  ([height width m [x y] include-diagonal?]
   (->> (coordinates->neighbor-coordinates height width [x y] include-diagonal?)
        (map (fn [[x y]] (get-cell m x y))))))

(defn two-dimensional-map
  "Given a function f, a two directional map m (noise/game map), a height, and a width, applies f to
   every tile within m. The function f should take 3 arguments: the value of the cell, its x
   coordinate, and its y coordinate"
  [f m height width]
  (let [m* (to-array-2d m)]
    (for [y (range height)]
      (for [x (range width)]
        (f (aget m* y x) x y)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Printing maps
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
  [{:keys [terrain-type resource]} color?]
  (let [color (or (when color? (terrain-type->color terrain-type)) color/white)
        output-char (or (:text-symbol resource) (terrain-type->char terrain-type) "?")]
    (color output-char)))

(defn print-map
  "Given a game map, prints it out in a readable way."
  ([game-map]
   (print-map game-map false))

  ([game-map color?]
   (doseq [row game-map]
     (doseq [tile row]
       (print (tile->printable-char tile color?)))
     (println))))

