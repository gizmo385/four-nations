(ns four-nations.core
  (:require
    [four-nations.model.map :as m]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.model.map.enrichments :as enrichments]
    [four-nations.general.types :as types]
    [reagent.core :as reagent :refer [atom]]
    [reagent-modals.modals :as reagent-modals]
    [goog.string :as gstring]
    [goog.string.format]
    [reagent.dom :as rd]))

(enable-console-print!)

(defonce game-generation-settings
  ;; This repesents the settings that the map will be generated using.
  (atom {:enrichments enrichments/default-enrichment-options
         :map-height 50
         :map-width 50
         :smoothing-passes 15}))

(defonce game-state
  ;; This represents the state of the actual game
  (atom {:game-map nil
         :dimension nil
         :tile-size 32}))

(defn tile-detail
  [tile]
  [:div
   {:style {:display :inline-block
            :border "1px solid black"
            :margin :auto}}
   [:p (gstring/format "Tile @ (%s, %s)" (-> tile :point :x) (-> tile :point :y))]
   [:p (gstring/format "Terrain Type: %s" (-> tile :attributes :terrain-type name))]
   [:p (gstring/format "Biome: %s" (-> tile :attributes :biome :display-name))]
   [:p (gstring/format "Resource: %s" (get-in tile [:attributes :resource :display-name] "None"))]
   ])

(def biome-terrain-type-image-config
  {:tundra {:land "images/tiles/ice.png"
            :water "images/tiles/water.png"
            :coast "images/tiles/coast.png"
            }
   :forest {:land "images/tiles/dark-grass.png"
            :water "images/tiles/water.png"
            :coast "images/tiles/coast.png"
            }
   :desert {:land "images/tiles/sand.png"
            :water "images/tiles/water.png"
            :coast "images/tiles/coast.png"
            }
   :plains {:land "images/tiles/grass.png"
            :water "images/tiles/water.png"
            :coast "images/tiles/coast.png"}
   nil {:land "images/tiles/grass.png"
        :water "images/tiles/water.png"
        :coast "images/tiles/coast.png"}})

(def resource-image-config
  {:oak-tree "images/resources/oak-tree.png"
   :fir-tree "images/resources/fir-tree.png"
   :gold-vein "images/resources/gold.png"
   :oil-deposit "images/resources/oil.png"})

(defn tile-resource-image
  [tile tile-size]
  (if-let [resource-image (-> tile :attributes :resource :name resource-image-config)]
    [:img {:style {:position :absolute}
           :src resource-image}]))

(defn tile-terrain-image
  [tile tile-size]
  (let [terrain-type (get-in tile [:attributes :terrain-type])
        biome (get-in tile [:attributes :biome :name])]
    (if-let [image (get-in biome-terrain-type-image-config [biome terrain-type])]
      [:img {:style {:position :absolute
                     :height tile-size :width tile-size}
             :src image}])))

(defn map-tile
  "Render a single map tile"
  [t]
  (let [coordinates (:point t)
        tile-size (:tile-size @game-state)]
    [:a
     {:style {:height tile-size
              :width tile-size
              :border "1px solid black"
              :display :inline-block
              :font-family "monospace"
              :background-color (-> t :attributes :terrain-type map-utils/terrain-type->color)}
      :on-click (fn [_] (reagent-modals/modal! [tile-detail t]))}
     [tile-terrain-image t tile-size]
     [tile-resource-image t tile-size]]))

(defn game-map-display
  "Render the entire map based on the current state"
  [game-map dimension]
  [:div
   (for [y (range (:height dimension))]
     ^{:key (gstring/format "GameMapRow_y=%s" y)}
     [:div
      {:class "map-row"}
      (for [x (range (:width dimension))]
        ^{:key (gstring/format "GameMapTile_y=%s_x=%s" y x)}
        [map-tile (-> game-map (map-utils/get-cell (types/->Point x y)))])])])


(defn re-generate-map
  "Handler for re-generating the map based on the current settings"
  [_]
  (let [{:keys [:enrichments :map-height :map-width :smoothing-passes] :as settings} @game-generation-settings
        dimension (types/->Dimension map-height map-width)]
    (->> (time (m/build-map dimension smoothing-passes enrichments))
         (swap! game-state assoc :dimension dimension :game-map))))

(defn update-generation-setting
  "Updates the map generation settings based on an event from a range input"
  [attribute-to-update event type-fn]
  (let [new-value (type-fn (.. event -target -value))]
    (swap! game-generation-settings assoc-in attribute-to-update new-value)))

(defn labeled-settings-slider
  "A type-aware slider element that is labeled with the current value"
  [label attr-path min-value max-value step clj-type-fn js-type-fn]
  (let [value (get-in @game-generation-settings attr-path)]
    [:div
     {:style {:width "50%"}}
     label ": "
     [:input
      {:type "range"
       :value value
       :min min-value
       :max max-value
       :step step
       :on-change (fn [e] (update-generation-setting attr-path e js-type-fn)) }]
     (clj-type-fn value)]))

(defn game-setting-controls
  "UI elements for altering the generation settings of the map"
  []
  [:div
   {:style {:margin-bottom "20px"}}
   [:h3 "Game Settings"]

   [labeled-settings-slider "Height" [:map-height] 10 500 1 int js/parseInt]
   [labeled-settings-slider "Width" [:map-width] 10 500 1 int js/parseInt]
   [labeled-settings-slider "Smoothing Passes" [:smoothing-passes] 1 30 1 int js/parseInt]
   [labeled-settings-slider
    "Water Spread Chance" [:enrichments :water-spread-chance] 0.01 1 0.01 float js/parseFloat]
   [labeled-settings-slider "Water Border" [:enrichments :water-border] 1 10 1 int js/parseInt]
   [labeled-settings-slider "Biome Count" [:enrichments :biome-count] 1 50 1 int js/parseInt]

   [:button.green {:on-click re-generate-map} "Re-generate Map"]])

(defn game-map-or-loading
  []
  ;; TODO: Add a loading mechanism when a new map is being generated
  (let [{:keys [game-map dimension]} @game-state]
    [game-map-display game-map dimension]))


(defn game-display []
  (let []
    [:div
     [reagent-modals/modal-window]
     [game-setting-controls]
     [game-map-or-loading]]))

(rd/render [game-display]
           (. js/document (getElementById "app")))

(defn on-js-reload []
  ;; optionally touch your game-generation-settings to force rerendering depending on
  ;; your application
  ;; (swap! game-generation-settings update-in [:__figwheel_counter] inc)
  )
