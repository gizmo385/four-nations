(ns four-nations.view.gui.images
  (:require
    [clojure.java.io :as io])
  (:import
    [javafx.scene.image Image]))

(defn load-image*
  "Loads an image from from a filename and returns the JavaFX image for it."
  [filename]
  (println (format "Attempting to load image at %s" filename))
  (new Image filename))

(def
  ^{:arglists '([filename])}
  load-image
  "Memoized version of load-image* that loads an image at a particular filename."
  (memoize load-image*))
