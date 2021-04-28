(ns four-nations.model.map.noise-map
  (:require
    [four-nations.general.utils :as utils]
    [four-nations.model.map.utils :as map-utils]))

(defrecord NoiseMap [noise height width])

(defn generate-2d-noise
  "Given a height and a width, generates a two-dimensional array of noise, where each cell is
   occupied by a number between 1 and 255 (inclusive on both sides)."
  [height width]
  (letfn [(make-cell [] (inc (rand-int 255)))
          (make-row [] (repeatedly width make-cell))]
    (repeatedly height make-row)))

(defn average-of-neighbors
  "Given a 2-dimensional array of noise, a height, width, and a cell coordinate, calculates the
   average of the values of its neighbor cells"
  [noise height width x y]
  (let [neighbors (map-utils/coordinates->neighbors height width noise [x y])]
    (quot (apply + neighbors)
          (count neighbors))))

(defn average-2d-noise
  "Given a 2-dimensional array of noise, returns a 2-dimensional array of averaged noise, where the
   value of each cell is the average of the values of its neighbors."
  [noise height width]
  (for [y (range height)]
    (for [x (range width)]
      (average-of-neighbors noise height width x y))))

(defn smooth-noisemap
  [noise height width smoothing-passes]
  (utils/n-times
    #(average-2d-noise % height width)
    noise
    smoothing-passes))

(defn add-water-border
  [noise height width water-border]
  (map-utils/two-dimensional-map
    (fn [v x y]
      (if (or (< x water-border)
              (> x (- width water-border))
              (< y water-border)
              (> y (- height water-border)))
        0
        v))
    noise
    height
    width))

(defn generate-noisemap
  "Given a height and width, generates a 2-dimensional map of averaged noise, which can be used for
   procedural terrain generation. You can optionally supply the number of times that the noise will
   be smoothed out (defaults to once)."
  ([height width]
   (generate-noisemap height width 1 0))

  ([height width smoothing-passes]
   (generate-noisemap height width smoothing-passes 0))

  ([height width smoothing-passes water-border]
   (-> (generate-2d-noise height width)
       (smooth-noisemap height width smoothing-passes)
       (add-water-border height width water-border)
       (->NoiseMap height width))))
