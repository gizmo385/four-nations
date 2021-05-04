(ns four-nations.view.gui.core
  (:require
    [cljfx.api :as fx]
    [four-nations.model.map :as m]
    [four-nations.general.utils :as utils]
    [four-nations.model.map.utils :as map-utils])
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
                     tile (map-utils/get-cell (:game-map game-map) x y)]
                 (doto (.getGraphicsContext2D canvas)
                   (.setStroke Color/BLACK)
                   (.setLineWidth 1)
                   (.setFill (-> tile :terrain-type terrain-type->color))
                   (.fillRect draw-x draw-y tile-size tile-size)
                   (.strokeRect draw-x draw-y tile-size tile-size))))))})


(defn -main [& args]
  (Platform/setImplicitExit true)
  (let [
        ;game-map (m/build-map 50 50)
        game-map (m/build-map 120 250)
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
                                    :width 5000
                                    :height 5000
                                    :game-map game-map
                                    :tile-size 10}]}}}))))
