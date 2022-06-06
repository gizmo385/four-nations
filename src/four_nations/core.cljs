(ns four-nations.core
  (:require
    [four-nations.model.map :as m]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.model.map.enrichments :as enrichments]
    [four-nations.general.types :as types]
    [reagent.core :as reagent :refer [atom]]
    [goog.string :as gstring]
    [reagent.dom :as rd]))

(enable-console-print!)

(defonce game-generation-settings
  ;; This repesents the settings that the map will be generated using.
  (atom {:enrichments enrichments/default-enrichment-options
         :map-height 50
         :map-width 150
         :smoothing-passes 15}))

(defonce game-state
  ;; This represents the state of the actual game
  (atom {:map nil
         :dimension nil
         :tile-size 30}))

(defn map-tile
  "Render a single map tile"
  [t]
  (let [coordinates (:point t)
        tile-size (:tile-size @game-state)]
    [:a
     {:style {:height tile-size
              :width tile-size
              :border "1px solid black"
              :font-family "monospace"
              :background-color (-> t :attributes :terrain-type map-utils/terrain-type->color)}}
     (map-utils/tile->printable-char t)]))

(defn game-map-display
  "Render the entire map based on the current state"
  []
  (if-let [{:keys [game-map dimension]} @game-state]
    [:div
     (for [y (range (:height dimension))]
       ^{:key (gstring/format "GameMapRow_y=%s" y)}
       [:div
        (for [x (range (:width dimension))]
          ^{:key (gstring/format "GameMapTile_y=%s_x=%s" y x)}
          [map-tile
           (-> game-map (map-utils/get-cell (types/->Point x y)))])
        [:br]])]))


(defn re-generate-map
  "Handler for re-generating the map based on the current settings"
  [_]
  (let [{:keys [:enrichments :map-height :map-width :smoothing-passes] :as settings} @game-generation-settings
        dimension (types/->Dimension map-height map-width)]
    (println "Generating new map with settings: " settings)
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
     label ": " (clj-type-fn value)
     [:input
      {:type "range"
       :value value
       :min min-value
       :max max-value
       :step step
       :on-change (fn [e] (update-generation-setting attr-path e js-type-fn)) }]]))

(defn game-setting-controls
  "UI elements for altering the generation settings of the map"
  []
  [:div
   {:style {:width "200px"
            :margin-bottom "20px"}}
   [:h3 "Game Settings"]

   [labeled-settings-slider "Height" [:map-height] 10 500 1 int js/parseInt]
   [labeled-settings-slider "Width" [:map-width] 10 500 1 int js/parseInt]
   [labeled-settings-slider "Smoothing Passes" [:smoothing-passes] 1 30 1 int js/parseInt]
   [labeled-settings-slider
    "Water Spread Chance" [:enrichments :water-spread-chance] 0.01 1 0.01 float js/parseFloat]
   [labeled-settings-slider "Water Border" [:enrichments :water-border] 1 10 1 int js/parseInt]
   [labeled-settings-slider "Biome Count" [:enrichments :biome-count] 1 50 1 int js/parseInt]

   [:button.green {:on-click re-generate-map} "Re-generate Map"]])


(defn game-display []
  (let []
    [:div
     [game-setting-controls]
     [game-map-display]]))

(rd/render [game-display]
           (. js/document (getElementById "app")))

(defn on-js-reload []
  ;; optionally touch your game-generation-settings to force rerendering depending on
  ;; your application
  ;; (swap! game-generation-settings update-in [:__figwheel_counter] inc)
  )
