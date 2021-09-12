(ns four-nations.view.gui.core
  (:require
    [cljfx.api :as fx]
    [clojure.core.cache :as cache]
    [four-nations.general.types :refer [->Dimension ->Point]]
    [four-nations.general.utils :as utils]
    [four-nations.model.map :as m]
    [four-nations.model.units.core :as units]
    [four-nations.model.game :as game]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.view.gui.images :as images]
    [random-seed.core :as rs])
  (:import
    [javafx.application Platform]
    [javafx.stage Screen]
    [javafx.scene.canvas Canvas]
    [javafx.scene.paint Color]
    [javafx.scene.shape Rectangle]
    [javafx.scene.input KeyCode]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Top-level state definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord MapViewPort [x y])

(def *state
  (atom (fx/create-context
          {:viewport (->MapViewPort 0 0)
           :tile-size 32
           :height 500
           :width 500
           :dimension nil
           :game-init-strat nil
           :game nil}
          cache/lru-cache-factory)))

(defn update-tile-size!
  "Updates the tile size, which specifies how large the tiles will be drawn, based on the supplied
   delta value."
  [delta]
  (letfn [(update-ts [ts] (max 1 (min 64 (+ ts delta))))]
    (swap! *state fx/swap-context update :tile-size update-ts)))

(defn translate-viewport!
  "Moves the current viewport in a by the specifies delta values.

   TODO: Currently, it's possible to move the viewport _past_ the sides of the map. This is
   something that should be fixed, but is not a large issue at the moment."
  [{:keys [x y] :as viewport} delta-x delta-y map-height map-width]
  (let [new-x (max 0 (min (+ x delta-x) map-width))
        new-y (max 0 (min (+ y delta-y) map-height))
        new-viewport (assoc viewport :x new-x :y new-y)]
    (swap! *state fx/swap-context assoc :viewport new-viewport)))

(defn set-viewport!
  "Sets the viewport center to the specified location"
  [{:keys [x y] :as viewport} {:keys [height width] :as map-dimensions}]
  (let [new-viewport (assoc viewport :x (max 0 (min x width)) :y (max 0 (min y height)))]
    (swap! *state fx/swap-context assoc :viewport new-viewport)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Image loading
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def tilesets (-> "tilesets.edn" utils/load-edn-resource images/config->tilesets))
(def biome->terrain-types->image
  (utils/load-edn-resource "image-configuration.edn"))

(defn tile->terrain-image*
  "Given a tile, determines the image that should be painted for the terrain on that tile."
  [tile]
  (let [terrain-type (get-in tile [:attributes :terrain-type])
        biome (get-in tile [:attributes :biome :name])
        image-key (get-in biome->terrain-types->image [biome terrain-type])]
    (cond
      (keyword? image-key) (images/random-tile-from-tilesets tilesets image-key)
      (string? image-key) (images/load-image image-key))))

(def tile->terrain-image (memoize tile->terrain-image*))

(defn regenerate-game!
  [strategy]
  (let [new-game (game/initialize-game strategy)
        {:keys [height width] :as map-dimensions} (:map-dimensions new-game)]
    (swap! *state fx/swap-context assoc :game new-game)
    ;; TODO: Right now, this sets the top left corner to one of the units, but it should center on
    ;; the units instead. This means we need to calculate how to put the tile in the middle, or as
    ;; close as possible
    #_(set-viewport! (-> new-game :units keys rs/rand-nth) (:map-dimensions new-game))
    (set-viewport! (->Point (int (quot height 2)) (int (quot width 2))) map-dimensions)))

(defmulti event-handler
  "A multimethod defining how to handle events occurring within the map display"
  :event/type)

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; The Canvas element
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn clear-canvas!
  "Wipes the current graphics display on the supplied canvas"
  [canvas]
  (let [graphics (.getGraphicsContext2D canvas)
        height (.getHeight canvas)
        width (.getWidth canvas)]
    (.clearRect graphics 0 0 width height)))

(defn draw-image
  "For a tile at a particular spot on the graphics canvas, draw images on that position."
  [canvas tile maybe-unit x y tile-size]
  (let [graphics (.getGraphicsContext2D canvas)
        tile-image (tile->terrain-image tile)]
    (doto graphics
      (.drawImage tile-image x y tile-size tile-size))
    (when-let [resource-image (get-in tile [:attributes :resource :image])]
      (doto graphics
        (.drawImage (images/load-image resource-image) x y tile-size tile-size)))
    (when-let [unit-image (some-> maybe-unit :unit-type :image images/load-image)]
      (doto graphics
        (.drawImage unit-image x y tile-size tile-size)))))

(defn draw-map
  "Given a canvas and information about the game map, draws the map onto the canvas."
  [canvas game tile-size viewport]
  (clear-canvas! canvas)
  (let [canvas-height (.getHeight canvas)
        canvas-width (.getWidth canvas)
        game-map-width (-> game :map-dimension :width)
        game-map-height (-> game :map-dimension :height)
        ;; We need to calculate how many tiles we can actually draw. We calculate this by dividing
        ;; the size of the canvas in pixels by the size of an individual tile in pixels. This tells
        ;; us how many cells, from our starting position, we need to draw.
        horizontal-viewport-size (quot canvas-width tile-size)
        vertical-viewport-size (quot canvas-height tile-size)]
    (doseq [x-index (range horizontal-viewport-size)]
      (doseq [y-index (range vertical-viewport-size)]
        (when-let [tile (map-utils/get-cell
                          (:game-map game)
                          (+ x-index (:x viewport))
                          (+ y-index (:y viewport)))]
          (let [tile-point (:point tile)
                maybe-unit (get-in game [:units tile-point])]
            (draw-image
              canvas tile maybe-unit (* x-index tile-size) (* y-index tile-size) tile-size)))))))

(defn canvas-map
  "Defines a JavaFX canvas component that draws the game map."
  [{:keys [fx/context max-height max-width]}]
  (let [game (fx/sub-val context :game)
        tile-size (fx/sub-val context :tile-size)
        viewport (fx/sub-val context :viewport)]
    {:fx/type :canvas
     :height max-width
     :width max-height
     :on-mouse-clicked {:event/type ::tile-clicked
                              :viewport viewport
                              :game game
                              :tile-size tile-size}
     :draw (fn [^Canvas canvas]
             (draw-map canvas game tile-size viewport))}))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Event handler
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defmethod event-handler ::key-pressed
  [{:keys [fx/context fx/event viewport map-height map-width game-init-strat]}]
  (let [key-code (.getCode event)]
    (condp = key-code
      ;; Moving up/down/left/right across the map
      KeyCode/W (translate-viewport! viewport 0 -1 map-height map-width)
      KeyCode/A (translate-viewport! viewport -1 0 map-height map-width)
      KeyCode/S (translate-viewport! viewport 0 1 map-height map-width)
      KeyCode/D (translate-viewport! viewport 1 0 map-height map-width)

      ;; Regenerating the map
      KeyCode/R (regenerate-game! game-init-strat)

      ;; Zooming out/in on the map
      KeyCode/E (update-tile-size! 1)
      KeyCode/Q (update-tile-size! -1)
      nil)))

(defn canvas-click-location->map-tile-location
  "Given a mouse click event from within the map canvas, determines which coordinates on the game
   map that click originated from."
  [click-event viewport tile-size]
  (let [click-x (.getX click-event)
        click-y (.getY click-event)
        tile-x (-> click-x (quot tile-size) (+ (:x viewport)) int)
        tile-y (-> click-y (quot tile-size) (+ (:y viewport)) int)]
    (->Point (-> click-x (quot tile-size) (+ (:x viewport)) int)
             (-> click-y (quot tile-size) (+ (:y viewport)) int))))

(defmethod event-handler ::tile-clicked
  [{:keys [fx/event game viewport tile-size]}]
  (let [tile-location (canvas-click-location->map-tile-location event viewport tile-size)
        clicked-cell (map-utils/get-cell (:game-map game) tile-location)]
    (println clicked-cell)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Map display stage definition
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn map-display-stage
  "A JavaFX stage that draws the map.

   TODO: As more elements of the game UI start to be created, this stage definition will likely need
   to be simplified, and this function will only include the scene definition."
  [{:keys [fx/context dimension game-init-strat]}]
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
                              :map-height (:height dimension)
                              :map-width (:width dimension)
                              :game-init-strat game-init-strat}
             :root {:fx/type :v-box
                    :padding 10
                    :spacing 10
                    :children [{:fx/type canvas-map
                                :viewport viewport
                                :max-height max-height
                                :max-width max-width}]}}}))



(defn build-renderer
  "Given some information about the map, we'll build a renderer that the map-display stage/scene."
  [dimension strategy]
  (fx/create-renderer
    :middleware (comp
                  fx/wrap-context-desc
                  (fx/wrap-map-desc (fn [_] {:fx/type map-display-stage
                                             :dimension dimension
                                             :game-init-strat strategy})))
    :opts {:fx.opt/map-event-handler event-handler
           :fx.opt/type->lifecycle #(or (fx/keyword->lifecycle %)
                                        (fx/fn->lifecycle-with-context %))}))

(defn -main [& args]
  (Platform/setImplicitExit true)
  (let [dimensions (->Dimension 100 100)
        unit-attributes (units/file->unit-attributes "unit-attributes.edn")
        unit-types (units/file->unit-types "unit-types.edn" unit-attributes)
        map-gen-strat (game/->MapGenerationStrategy dimensions 15 {})
        unit-spawn-strat (game/->UnitSpawnStrategy 5 (->Dimension 5 5) (:basic-worker unit-types))
        game-init-strat (game/->GameInitializationStrategy map-gen-strat unit-spawn-strat :water)
        view-renderer (build-renderer dimensions game-init-strat)]
    (swap! *state fx/swap-context assoc :game-init-strat game-init-strat)
    (regenerate-game! game-init-strat)
    (fx/mount-renderer *state view-renderer)))
