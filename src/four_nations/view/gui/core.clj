(ns four-nations.view.gui.core
  (:require
    [cljfx.api :as fx]
    [four-nations.model.map :as m]
    [four-nations.general.utils :as utils]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.view.gui.images :as images])
  (:import
    [javafx.application Application Platform]
    [javafx.scene.canvas Canvas]
    [javafx.scene.paint Color]
    [javafx.scene Group Scene]
    [javafx.scene.shape Rectangle]))

(def terrain-type->color
  {:water Color/BLUE
   :land Color/GREEN
   :coast Color/BEIGE
   :mountain Color/GREY})

(def terrain-type->image
  {:water "images/water.png"
   :land "images/grass.png"
   :mountain "images/mountain.png"
   :coast "images/coast.png"})

(defn draw-rectangle
  [canvas tile x y tile-size]
  (let [graphics (.getGraphicsContext2D canvas)]
    (doto graphics
      (.setFill (-> tile :terrain-type terrain-type->color))
      (.fillRect x y tile-size tile-size))
    (when (> tile-size 1)
      (doto graphics
        (.setStroke Color/BLACK)
        (.setLineWidth 1)
        (.strokeRect x y tile-size tile-size)))))

(defn draw-image
  [canvas tile x y tile-size]
  (let [graphics (.getGraphicsContext2D canvas)
        tile-image (-> tile :terrain-type terrain-type->image images/load-image)]
    (doto graphics
      (.drawImage tile-image x y tile-size tile-size))
    (when-let [resource-image (get-in tile [:resource :image])]
      (doto graphics
        (.drawImage (images/load-image resource-image) x y tile-size tile-size)))))

(defn draw-map
  [{:keys [width height tile-size game-map]}]
  {:fx/type :canvas
   :width width
   :height height
   :draw (fn [^Canvas canvas]
           (doseq [x (range (:width game-map))]
             (doseq [y (range (:height game-map))]
               (let [draw-x (* tile-size x)
                     draw-y (* tile-size y)
                     tile (map-utils/get-cell (:game-map game-map) x y)
                     tile-image (-> tile :terrain-type terrain-type->image images/load-image)
                     ]
                 (draw-image canvas tile draw-x draw-y tile-size)
                 ;(draw-rectangle canvas tile draw-x draw-y tile-size)
                 ))))})


(defn -main [& args]
  (Platform/setImplicitExit true)
  (let [
        ;game-map (m/build-map 120 250)
        game-map (m/build-map 100 200)
        ]
    (fx/on-fx-thread
      (fx/create-component
        {:fx/type :stage
         :showing true
         :title "Game Map"
         :scene {:fx/type :scene
                 :root {:fx/type :v-box
                        :padding 10
                        :spacing 10
                        :children [{:fx/type draw-map
                                    :width 1500
                                    :height 2000
                                    :game-map game-map
                                    :tile-size 10}]}}}))))
