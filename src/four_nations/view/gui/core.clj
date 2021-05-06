(ns four-nations.view.gui.core
  (:require
    [cljfx.api :as fx]
    [clojure.core.cache :as cache]
    [clojure.edn :as edn]
    [four-nations.model.map :as m]
    [four-nations.general.utils :as utils]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.view.gui.images :as images])
  (:import
    [javafx.application Application Platform]
    [javafx.stage Screen]
    [javafx.scene.canvas Canvas]
    [javafx.scene.paint Color]
    [javafx.scene Group Scene]
    [javafx.scene.shape Rectangle]
    [javafx.scene.input KeyCode]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; State and constants
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord MapViewPort [x y])

(def *state
  (atom (fx/create-context
          {:viewport (->MapViewPort 0 0)
           :tile-size 32
           :height 500
           :width 500
           :game-map nil}
          cache/lru-cache-factory)))

;; TODO: Maybe in a config file?
(def terrain-type->image
  {:water "images/water.png"
   :land "images/grass.png"
   :mountain "images/mountain.png"
   :coast "images/coast.png"})


(defmulti event-handler :event/type)

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; The Canvas element
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn clear-canvas!
  [canvas]
  (let [graphics (.getGraphicsContext2D canvas)
        height (.getHeight canvas)
        width (.getWidth canvas)]
    (.clearRect graphics 0 0 width height)))

(defn draw-image
  "For a tile at a particular spot on the graphics canvas, draw images on that position."
  [canvas tile x y tile-size]
  (let [graphics (.getGraphicsContext2D canvas)
        tile-image (-> tile :terrain-type terrain-type->image images/load-image)]
    (doto graphics
      (.drawImage tile-image x y tile-size tile-size))
    (when-let [resource-image (get-in tile [:resource :image])]
      (doto graphics
        (.drawImage (images/load-image resource-image) x y tile-size tile-size)))))

(defn draw-map
  "Given a canvas and information about the game map, draws the map onto the canvas."
  [canvas game-map tile-size viewport]
  (clear-canvas! canvas)
  (let [canvas-height (.getHeight canvas)
        canvas-width (.getWidth canvas)
        game-map-width (:width game-map)
        game-map-height (:height game-map)
        horizontal-viewport-size (quot canvas-width tile-size)
        vertical-viewport-size (quot canvas-height tile-size)]
    (doseq [x-index (range horizontal-viewport-size)]
      (doseq [y-index (range vertical-viewport-size)]
        (when-let [tile (map-utils/get-cell
                          (:game-map game-map)
                          (+ x-index (:x viewport))
                          (+ y-index (:y viewport)))]
          (draw-image canvas tile (* x-index tile-size) (* y-index tile-size) tile-size))))))

(defn canvas-map
  "Defines a JavaFX canvas component that draws the game map."
  [{:keys [fx/context max-height max-width]}]
  (let [game-map (fx/sub-val context :game-map)
        tile-size (fx/sub-val context :tile-size)
        viewport (fx/sub-val context :viewport)]
    {:fx/type :canvas
     :height max-width
     :width max-height
     :draw (fn [^Canvas canvas]
             (draw-map canvas game-map tile-size viewport))}))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Event handler
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn update-tile-size!
  [delta]
  (letfn [(update-ts [ts] (max 1 (min 64 (+ ts delta))))]
    (swap! *state fx/swap-context update :tile-size update-ts)))

(defn translate-viewport!
  [{:keys [x y] :as viewport} delta-x delta-y map-height map-width]
  (let [new-x (max 0 (min (+ x delta-x) map-width))
        new-y (max 0 (min (+ y delta-y) map-height))
        new-viewport (assoc viewport :x new-x :y new-y)]
    (swap! *state fx/swap-context assoc :viewport new-viewport)))

(defmethod event-handler ::key-pressed
  [{:keys [fx/event viewport map-height map-width]}]
  (let [key-code (.getCode event)]
    (condp = key-code
      KeyCode/W (translate-viewport! viewport 0 -1 map-height map-width)
      KeyCode/A (translate-viewport! viewport -1 0 map-height map-width)
      KeyCode/S (translate-viewport! viewport 0 1 map-height map-width)
      KeyCode/D (translate-viewport! viewport 1 0 map-height map-width)
      KeyCode/E (update-tile-size! 1)
      KeyCode/Q (update-tile-size! -1)
      nil)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Root scene definition
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn root
  [{:keys [fx/context map-height map-width]}]
  (let [bounds (-> (Screen/getPrimary) .getBounds)
        max-height (.getHeight bounds)
        max-width (.getWidth bounds)
        viewport (fx/sub-val context :viewport)]
    {:fx/type :stage
     :showing true
     :title "Game Map"
     :max-height max-height
     :max-width max-width
     :scene {:fx/type :scene
             :on-key-pressed {:event/type ::key-pressed
                              :viewport viewport
                              :map-height map-height
                              :map-width map-width}
             :root {:fx/type :v-box
                    :padding 10
                    :spacing 10
                    :children [{:fx/type canvas-map
                                :max-height max-height
                                :max-width max-width}]}}}))

(defn build-renderer
  [map-height map-width]
  (fx/create-renderer
    :middleware (comp
                  fx/wrap-context-desc
                  (fx/wrap-map-desc (fn [_] {:fx/type root
                                             :map-height map-height
                                             :map-width map-width})))
    :opts {:fx.opt/map-event-handler event-handler
           :fx.opt/type->lifecycle #(or (fx/keyword->lifecycle %)
                                        (fx/fn->lifecycle-with-context %))}))

(defn -main [& args]
  (Platform/setImplicitExit true)
  (let [map-height 100
        map-width 100
        game-map (m/build-map map-height map-width {:smoothing-passes 15})
        view-renderer (build-renderer map-height map-width)]
    (swap! *state fx/swap-context assoc :game-map game-map)
    (fx/mount-renderer *state view-renderer)))
