(ns four-nations.view.gui.images
  (:require
    [clojure.java.io :as io]
    [four-nations.general.utils :as utils]
    [random-seed.core :as rs])
  (:import
    [javax.imageio ImageIO]
    [javafx.embed.swing SwingFXUtils]
    [javafx.scene.image Image ImageView]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Loading normal images
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn load-image*
  "Loads an image from from a filename and returns the JavaFX image for it."
  [filename]
  (println (format "Loading image at %s" filename))
  (new Image filename))

(def
  ^{:arglists '([filename])}
  load-image
  "Memoized version of load-image* that loads an image at a particular filename."
  (memoize load-image*))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Images from tilesets
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn tile-coordinates->subimage-coordinates
  [tiles tile-size]
  (zipmap
    (keys tiles)
    (map (fn [coordinates] (for [[x y] coordinates] [(* tile-size x) (* tile-size y)]))
         (vals tiles))))

(defn valid-tile-definitions?
  [tiles tileset-image-height tileset-image-width]
  (every?
    (fn [[_ coordinates]]
      (every?
        (fn [[x y]] (and (<= 0 x tileset-image-width) (<= 0 y tileset-image-height)))
        coordinates))
    tiles))

(defn load-subimage*
  "Extracts and returns a smaller portion of a larger image."
  [image x y tile-size]
  (.getSubimage image x y tile-size tile-size))

(def load-subimage
  "Extracts and returns a smaller portion of a larger image. This function is memoized."
  ^{:arglists '([image x y tile-size])}
  (memoize load-subimage*))

(defn load-tiles-from-tileset
  "Given a tileset definition, which "
  [{:keys [file tiles tile-size] :as tileset}]
  (let [tileset-image (-> file io/resource io/file ImageIO/read)
        tileset-image-height (.getHeight tileset-image)
        tileset-image-width (.getWidth tileset-image)
        subimage-tiles (tile-coordinates->subimage-coordinates tiles tile-size)]
    ;; Image bounds checking
    (when-not (valid-tile-definitions? subimage-tiles tileset-image-height tileset-image-width)
      (throw (ex-info "Invalid tileset definition. Some coordinates go beyond image boundaries.")))

    (zipmap
      (keys subimage-tiles)
      (map (fn [coordinates] (for [[x y] coordinates] (load-subimage tileset-image x y tile-size)))
           (vals subimage-tiles)))))

(defn buffered-image->javafx-image
  [i]
  (SwingFXUtils/toFXImage i nil))

(defn config->tilesets
  [tileset-configuration]
  (apply merge-with concat (map load-tiles-from-tileset tileset-configuration)))

(defn random-tile-from-tilesets
  [tilesets group]
  (-> tilesets (get group) rs/rand-nth buffered-image->javafx-image))
