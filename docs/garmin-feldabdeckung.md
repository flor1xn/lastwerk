# Garmin-Feldabdeckung (Ergebnis der Sonde, L1)

Stand: 2026-10-14, Bibliothek garminconnect 0.3.17. Erzeugt aus `local/probe/probe-report.json`. Der Bericht enthält Feldnamen und Datentypen, **keine Werte** aus dem Konto.

## Zusammenfassung

- Aufrufe: 271, ok 255, Fehler 14, gesperrt 0, Zeitlimit 0, Wiederholungen 2.
- Drosselung der Sonde: Mindestabstand 1.5 s plus bis 0.5 s Zufall. Es gab keine 429 oder 403 und keine Sperre (O24, O25: für diesen Lauf kein Hinweis auf Drosselung, Startwert bleibt vorsichtig).
- Status: `ok` = Aufruf gelang mit Daten, `empty` = Aufruf gelang ohne Daten im Testzeitraum, `error` = Aufruf scheiterte.
- Spanne: größte geprüfte Bereichsspanne in Tagen. 1792 ist die größte geprüfte, dort wurde kein Limit gefunden. Kleinere Werte sind die Grenze, über der der Aufruf mit `GarminConnectConnectionError` scheiterte.
- Historie: frühestes Jahr mit Daten (auf das Jahr gerundet; das genaue Datum würde den Beginn des Kontos verraten, T38).

## Übersicht

| Name | Methode | Klasse | Status | Spanne | Historie ab | Felder |
|---|---|---|---|---|---|---|
| profile | get_user_profile | S1 | ok |  |  | 6 |
| settings | get_userprofile_settings | S1 | ok |  |  | 17 |
| devices | get_devices | S1 | ok |  |  | 2 |
| device_last_used | get_device_last_used | S1 | ok |  |  | 8 |
| primary_training_device | get_primary_training_device | S1 | ok |  |  | 5 |
| zones | get_heart_rate_zones | S1 | ok |  |  | 2 |
| power_zones | get_power_zones | S1 | ok |  |  | 2 |
| ftp | get_cycling_ftp | S1 | ok |  |  | 8 |
| lactate_threshold | get_lactate_threshold | S1 | ok |  |  | 2 |
| personal_records | get_personal_record | S1 | ok |  |  | 2 |
| goals | get_goals | S1 | empty |  |  | 8 |
| workouts | get_workouts | S1 | ok |  |  | 2 |
| training_plans | get_training_plans | S1 | ok |  |  | 2 |
| activity_types | get_activity_types | S1 | ok |  |  | 2 |
| gear | get_gear | S1 | ok |  |  | 2 |
| activities_list | get_activities | S1 | ok |  |  | 2 |
| summary | get_activity | S1 | ok |  |  | 13 |
| details | get_activity_details | S1 | ok |  |  | 10 |
| splits | get_activity_splits | S1 | ok |  |  | 3 |
| typed_splits | get_activity_typed_splits | S1 | ok |  |  | 3 |
| split_summaries | get_activity_split_summaries | S1 | ok |  |  | 3 |
| weather | get_activity_weather | S1 | ok |  |  | 13 |
| hr_zones | get_activity_hr_in_timezones | S1 | ok |  |  | 2 |
| power_zones_activity | get_activity_power_in_timezones | S1 | empty |  |  | 8 |
| exercise_sets | get_activity_exercise_sets | S1 | ok |  |  | 2 |
| gear_activity | get_activity_gear | S1 | ok |  |  | 2 |
| original | download_activity | S1 | ok |  |  | 13 |
| user_summary | get_user_summary | S2 | ok |  |  | 15 |
| steps | get_steps_data | S2 | ok |  |  | 2 |
| floors | get_floors | S2 | ok |  |  | 6 |
| heart_rates | get_heart_rates | S2 | ok |  | 2010 | 12 |
| rhr | get_rhr_day | S2 | ok |  | 2010 | 5 |
| stress | get_stress_data | S2 | ok |  | 2010 | 14 |
| all_day_stress | get_all_day_stress | S2 | ok |  |  | 14 |
| body_battery_events | get_body_battery_events | S2 | ok |  |  | 2 |
| respiration | get_respiration_data | S2 | ok |  |  | 24 |
| spo2 | get_spo2_data | S2 | ok |  |  | 26 |
| intensity_minutes | get_intensity_minutes_data | S2 | ok |  |  | 17 |
| sleep | get_sleep_data | S2 | ok |  | 2010 | 20 |
| hrv | get_hrv_data | S2 | ok |  | 2025 | 11 |
| training_readiness | get_training_readiness | S2 | ok |  | 2025 | 2 |
| training_status | get_training_status | S2 | ok |  | 2010 | 5 |
| daily_training_status | get_daily_training_status | S2 | ok |  |  | 5 |
| morning_training_readiness | get_morning_training_readiness | S2 | ok |  |  | 29 |
| fitness_age | get_fitnessage_data | S2 | ok |  |  | 6 |
| max_metrics | get_max_metrics | S2 | empty |  | 2026 | 8 |
| four_week_load_balance | get_training_four_week_load_balance | S2 | ok |  |  | 3 |
| activities_by_date | get_activities_by_date | S1 | ok | 1792 | 2025 | 2 |
| body_battery | get_body_battery | S2 | ok | 31 |  | 2 |
| hrv_range | get_hrv_data_range | S2 | ok | 336 | 2025 | 2 |
| endurance_score | get_endurance_score | S2 | ok | 392 |  | 7 |
| hill_score | get_hill_score | S2 | ok | 392 |  | 6 |
| running_tolerance | get_running_tolerance | S2 | empty | 336 |  | 8 |
| race_predictions | get_race_predictions | S2 | error (ValueError) |  |  | 0 |
| max_metrics_range | get_max_metrics_range | S2 | ok | 1792 |  | 2 |
| daily_steps | get_daily_steps | S2 | ok | 1792 |  | 2 |
| sleep_daily | get_sleep_daily | S2 | ok | 1792 | 2025 | 2 |
| rhr_daily | get_rhr_daily | S2 | ok | 1792 | 2025 | 2 |
| calories_daily | get_calories_daily | S2 | ok | 1792 |  | 2 |
| training_load_activities | get_training_load_activities | S2 | ok | 1792 | 2025 | 2 |
| weekly_intensity_minutes | get_weekly_intensity_minutes | S2 | ok | 336 |  | 2 |
| weigh_ins | get_weigh_ins | S3 | ok | 1792 |  | 4 |
| body_composition | get_body_composition | S3 | ok | 1792 |  | 4 |
| blood_pressure | get_blood_pressure | S3 | ok | 1792 |  | 4 |
| hydration | get_hydration_data | S3 | ok |  |  | 8 |
| nutrition_food_log | get_nutrition_daily_food_log | S3 | ok |  |  | 6 |
| nutrition_meals | get_nutrition_daily_meals | S3 | ok |  |  | 2 |
| nutrition_settings | get_nutrition_daily_settings | S3 | empty |  |  | 0 |
| lifestyle_logging | get_lifestyle_logging_data | S3 | ok |  |  | 3 |

## Felder je Datenart

Pfade mit Punkt, Arrays als `array(n) / Elementtyp`. Tiefe wie von der Sonde erfasst.

### profile (`get_user_profile`, ok)

- `id`: integer
- `userData.gender`: string
- `userData.weight`: number
- `userData.height`: number
- `userData.timeFormat`: string
- `userData.birthDate`: string
- `userData.measurementSystem`: string
- `userData.activityLevel`: null
- `userData.handedness`: string
- `userData.powerFormat`: object(6 keys)
- `userData.heartRateFormat`: object(6 keys)
- `userData.firstDayOfWeek`: object(4 keys)
- `userData.vo2MaxRunning`: number
- `userData.vo2MaxCycling`: null
- `userData.lactateThresholdSpeed`: number
- `userData.lactateThresholdHeartRate`: integer
- `userData.diveNumber`: null
- `userData.intensityMinutesCalcMethod`: string
- `userData.moderateIntensityMinutesHrZone`: integer
- `userData.vigorousIntensityMinutesHrZone`: integer
- `userData.hydrationMeasurementUnit`: string
- `userData.hydrationContainers`: array(3) / object(3 keys)
- `userData.hydrationAutoGoalEnabled`: boolean
- `userData.firstbeatMaxStressScore`: integer
- `userData.firstbeatCyclingLtTimestamp`: integer
- `userData.firstbeatRunningLtTimestamp`: integer
- `userData.thresholdHeartRateAutoDetected`: boolean
- `userData.ftpAutoDetected`: boolean
- `userData.trainingStatusPausedDate`: null
- `userData.weatherLocation`: object(6 keys)
- `userData.golfDistanceUnit`: string
- `userData.golfElevationUnit`: null
- `userData.golfSpeedUnit`: null
- `userData.externalBottomTime`: null
- `userData.availableTrainingDays`: array(7) / string
- `userData.preferredLongTrainingDays`: array(1) / string
- `userData.virtualCaddieDataSource`: null
- `userData.numberDivesAutomatically`: null
- `userData.firstbeatRowingLtTimestamp`: null
- `userData.lactateThresholdRowingPace`: null
- `userData.lactateThresholdHeartRateRowing`: null
- `userData.lactateThresholdHeartRateCycling`: null
- `userData.wtrPrimaryActivityType`: null
- `userSleep.sleepTime`: integer
- `userSleep.defaultSleepTime`: boolean
- `userSleep.wakeTime`: integer
- `userSleep.defaultWakeTime`: boolean
- `connectDate`: null
- `sourceType`: null
- `userSleepWindows`: array(8) / object(3 keys)

### settings (`get_userprofile_settings`, ok)

- `displayName`: string
- `preferredLocale`: string
- `measurementSystem`: string
- `firstDayOfWeek.dayId`: integer
- `firstDayOfWeek.dayName`: string
- `firstDayOfWeek.sortOrder`: integer
- `firstDayOfWeek.isPossibleFirstDay`: boolean
- `numberFormat`: string
- `timeFormat.formatId`: integer
- `timeFormat.formatKey`: string
- `timeFormat.minFraction`: integer
- `timeFormat.maxFraction`: integer
- `timeFormat.groupingUsed`: boolean
- `timeFormat.displayFormat`: string
- `dateFormat.formatId`: integer
- `dateFormat.formatKey`: string
- `dateFormat.minFraction`: integer
- `dateFormat.maxFraction`: integer
- `dateFormat.groupingUsed`: boolean
- `dateFormat.displayFormat`: string
- `powerFormat.formatId`: integer
- `powerFormat.formatKey`: string
- `powerFormat.minFraction`: integer
- `powerFormat.maxFraction`: integer
- `powerFormat.groupingUsed`: boolean
- `powerFormat.displayFormat`: null
- `heartRateFormat.formatId`: integer
- `heartRateFormat.formatKey`: string
- `heartRateFormat.minFraction`: integer
- `heartRateFormat.maxFraction`: integer
- `heartRateFormat.groupingUsed`: boolean
- `heartRateFormat.displayFormat`: null
- `timeZone`: string
- `hydrationMeasurementUnit`: string
- `hydrationContainers`: array(3) / object(3 keys)
- `golfDistanceUnit`: string
- `golfElevationUnit`: null
- `golfSpeedUnit`: null
- `availableTrainingDays`: null
- `preferredLongTrainingDays`: null

### devices (`get_devices`, ok)

- (Wurzel): array(2) / object(319 keys)

### device_last_used (`get_device_last_used`, ok)

- `userDeviceId`: integer
- `userProfileNumber`: integer
- `applicationNumber`: integer
- `lastUsedDeviceApplicationKey`: string
- `lastUsedDeviceName`: string
- `lastUsedDeviceUploadTime`: integer
- `imageUrl`: string
- `released`: boolean

### primary_training_device (`get_primary_training_device`, ok)

- `PrimaryTrainingDevice.deviceId`: integer
- `WearableDevices.deviceWeights`: array(1) / object(7 keys)
- `WearableDevices.wearableDeviceCount`: integer
- `TrainingStatusOnlyDevices.deviceWeights`: array(0)
- `PrimaryTrainingDevices.deviceWeights`: array(2) / object(7 keys)
- `PrimaryTrainingDevices.primaryTrainingDeviceCount`: integer
- `RegisteredDevices`: array(2) / object(319 keys)

### zones (`get_heart_rate_zones`, ok)

- (Wurzel): array(3) / {'trainingMethod': 'string', 'restingHeartRateUsed': 'integer', 'lactateThresholdHeartRateUsed': 'integer', 'zone1Floor': 'integer', 'zone2Floor': 'integer', 'zone3Floor': 'integer', 'zone4Floor': 'integer', 'zone5Floor': 'integer', 'maxHeartRateUsed': 'integer', 'restingHrAutoUpdateUsed': 'boolean', 'sport': 'string', 'changeState': 'string'}

### power_zones (`get_power_zones`, ok)

- (Wurzel): array(4) / {'sport': 'string', 'functionalThresholdPower': 'number', 'zone1Floor': 'number', 'zone2Floor': 'number', 'zone3Floor': 'number', 'zone4Floor': 'number', 'zone5Floor': 'number', 'zone6Floor': 'number', 'zone7Floor': 'number', 'userLocalTime': 'null'}

### ftp (`get_cycling_ftp`, ok)

- `userProfilePK`: integer
- `version`: integer
- `calendarDate`: string
- `isStale`: boolean
- `sequence`: integer
- `sport`: string
- `functionalThresholdPower`: integer
- `biometricSourceType`: string

### lactate_threshold (`get_lactate_threshold`, ok)

- `speed_and_heart_rate.userProfilePK`: integer
- `speed_and_heart_rate.version`: integer
- `speed_and_heart_rate.calendarDate`: string
- `speed_and_heart_rate.sequence`: integer
- `speed_and_heart_rate.speed`: number
- `speed_and_heart_rate.heartRate`: integer
- `speed_and_heart_rate.heartRateCycling`: null
- `power.userProfilePk`: integer
- `power.calendarDate`: string
- `power.origin`: string
- `power.sport`: string
- `power.functionalThresholdPower`: integer
- `power.weight`: number
- `power.powerToWeight`: number
- `power.ftpCreateTime`: string
- `power.weightCreateTime`: string
- `power.isStale`: boolean

### personal_records (`get_personal_record`, ok)

- (Wurzel): array(13) / {'id': 'integer', 'typeId': 'integer', 'status': 'string', 'activityId': 'integer', 'activityName': 'string', 'activityType': 'string', 'activityStartDateTimeInGMT': 'integer', 'actStartDateTimeInGMTFormatted': 'string', 'activityStartDateTimeLocal': 'integer', 'activityStartDateTimeLocalFormatted': 'string', 'value': 'number', 'prStartTimeGmt': 'integer', 'prStartTimeGmtFormatted': 'string', 'prStartTimeLocal': 'null', 'prStartTimeLocalFormatted': 'null', 'prTypeLabelKey': 'null', 'poolLengthUnit': 'null'}

### goals (`get_goals`, empty)

- (Wurzel): array(0)

### workouts (`get_workouts`, ok)

- (Wurzel): array(3) / {'workoutId': 'integer', 'ownerId': 'integer', 'workoutName': 'string', 'description': 'string', 'updateDate': 'string', 'createdDate': 'string', 'sportType': 'object(3 keys)', 'trainingPlanId': 'null', 'author': 'object(8 keys)', 'estimatedDurationInSecs': 'null', 'estimatedDistanceInMeters': 'number', 'estimateType': 'null', 'estimatedDistanceUnit': 'object(3 keys)', 'poolLength': 'number', 'poolLengthUnit': 'object(3 keys)', 'workoutProvider': 'string', 'workoutSourceId': 'string', 'consumer': 'string', 'atpPlanId': 'null', 'workoutNameI18nKey': 'null', 'descriptionI18nKey': 'null', 'workoutThumbnailUrl': 'string', 'shared': 'boolean', 'estimated': 'boolean'}

### training_plans (`get_training_plans`, ok)

- `trainingPlanList`: array(0)
- `searchFilter.ownerId`: integer
- `searchFilter.ownerDisplayName`: null
- `searchFilter.trainingLevels`: null
- `searchFilter.trainingStatusList`: array(0)
- `searchFilter.trainingTypes`: null
- `searchFilter.trainingSubTypes`: null
- `searchFilter.trainingVersions`: null
- `searchFilter.allowedPrivacyList`: null
- `searchFilter.locale`: null
- `searchFilter.start`: integer
- `searchFilter.limit`: integer
- `searchFilter.planKey`: boolean

### activity_types (`get_activity_types`, ok)

- (Wurzel): array(154) / {'typeId': 'integer', 'typeKey': 'string', 'parentTypeId': 'integer', 'isHidden': 'boolean', 'restricted': 'boolean', 'trimmable': 'boolean'}

### gear (`get_gear`, ok)

- (Wurzel): array(2) / {'gearPk': 'integer', 'uuid': 'string', 'userProfilePk': 'integer', 'gearMakeName': 'string', 'gearModelName': 'string', 'gearTypeName': 'string', 'gearStatusName': 'string', 'displayName': 'string', 'customMakeModel': 'string', 'imageNameLarge': 'null', 'imageNameMedium': 'null', 'imageNameSmall': 'null', 'dateBegin': 'string', 'dateEnd': 'null', 'maximumMeters': 'number', 'notified': 'boolean', 'createDate': 'string', 'updateDate': 'string'}

### activities_list (`get_activities`, ok)

- (Wurzel): array(20) / object(107 keys)

### summary (`get_activity`, ok)

- `activityId`: integer
- `activityUUID.uuid`: string
- `activityName`: string
- `userProfileId`: integer
- `isMultiSportParent`: boolean
- `activityTypeDTO.typeId`: integer
- `activityTypeDTO.typeKey`: string
- `activityTypeDTO.parentTypeId`: integer
- `activityTypeDTO.isHidden`: boolean
- `activityTypeDTO.restricted`: boolean
- `activityTypeDTO.trimmable`: boolean
- `eventTypeDTO.typeId`: integer
- `eventTypeDTO.typeKey`: string
- `eventTypeDTO.sortOrder`: integer
- `accessControlRuleDTO.typeId`: integer
- `accessControlRuleDTO.typeKey`: string
- `timeZoneUnitDTO.unitId`: integer
- `timeZoneUnitDTO.unitKey`: string
- `timeZoneUnitDTO.factor`: number
- `timeZoneUnitDTO.timeZone`: string
- `metadataDTO.isOriginal`: boolean
- `metadataDTO.deviceApplicationInstallationId`: integer
- `metadataDTO.agentApplicationInstallationId`: null
- `metadataDTO.agentString`: null
- `metadataDTO.fileFormat`: object(2 keys)
- `metadataDTO.associatedCourseId`: integer
- `metadataDTO.lastUpdateDate`: string
- `metadataDTO.uploadedDate`: string
- `metadataDTO.videoUrl`: null
- `metadataDTO.hasPolyline`: boolean
- `metadataDTO.hasChartData`: boolean
- `metadataDTO.hasHrTimeInZones`: boolean
- `metadataDTO.hasPowerTimeInZones`: boolean
- `metadataDTO.userInfoDto`: object(7 keys)
- `metadataDTO.childIds`: array(0)
- `metadataDTO.childActivityTypes`: array(0)
- `metadataDTO.sensors`: array(2) / object(7 keys)
- `metadataDTO.activityImages`: array(0)
- `metadataDTO.manufacturer`: string
- `metadataDTO.diveNumber`: null
- `metadataDTO.lapCount`: integer
- `metadataDTO.associatedWorkoutId`: null
- `metadataDTO.isAtpActivity`: null
- `metadataDTO.deviceMetaDataDTO`: object(3 keys)
- `metadataDTO.hasIntensityIntervals`: boolean
- `metadataDTO.hasSplits`: boolean
- `metadataDTO.eBikeMaxAssistModes`: null
- `metadataDTO.eBikeBatteryUsage`: null
- `metadataDTO.eBikeBatteryRemaining`: null
- `metadataDTO.eBikeAssistModeInfoDTOList`: null
- `metadataDTO.hasRunPowerWindData`: null
- `metadataDTO.calendarEventInfo`: null
- `metadataDTO.groupRideUUID`: null
- `metadataDTO.hasHeatMap`: boolean
- `metadataDTO.sessionResultType`: string
- `metadataDTO.hasMultiDayActivities`: boolean
- `metadataDTO.thirdPartyGymEquipment`: array(0)
- `metadataDTO.personalRecord`: boolean
- `metadataDTO.gcj02`: boolean
- `metadataDTO.runPowerWindDataEnabled`: null
- `metadataDTO.manualActivity`: boolean
- `metadataDTO.trimmed`: boolean
- `metadataDTO.autoCalcCalories`: boolean
- `metadataDTO.favorite`: boolean
- `metadataDTO.elevationCorrected`: boolean
- `summaryDTO.startTimeLocal`: string
- `summaryDTO.startTimeGMT`: string
- `summaryDTO.startLatitude`: number
- `summaryDTO.startLongitude`: number
- `summaryDTO.distance`: number
- `summaryDTO.duration`: number
- `summaryDTO.movingDuration`: number
- `summaryDTO.elapsedDuration`: number
- `summaryDTO.elevationGain`: number
- `summaryDTO.elevationLoss`: number
- `summaryDTO.maxElevation`: number
- `summaryDTO.minElevation`: number
- `summaryDTO.avgElevation`: number
- `summaryDTO.averageSpeed`: number
- `summaryDTO.averageMovingSpeed`: number
- … 33 weitere Felder

### details (`get_activity_details`, ok)

- `activityId`: integer
- `measurementCount`: integer
- `metricsCount`: integer
- `totalMetricsCount`: integer
- `metricDescriptors`: array(15) / object(3 keys)
- `activityDetailMetrics`: array(1528) / object(1 keys)
- `geoPolylineDTO.startPoint`: object(13 keys)
- `geoPolylineDTO.endPoint`: object(13 keys)
- `geoPolylineDTO.minLat`: number
- `geoPolylineDTO.maxLat`: number
- `geoPolylineDTO.minLon`: number
- `geoPolylineDTO.maxLon`: number
- `geoPolylineDTO.polyline`: array(1009) / object(13 keys)
- `heartRateDTOs`: null
- `pendingData`: null
- `detailsAvailable`: boolean

### splits (`get_activity_splits`, ok)

- `activityId`: integer
- `lapDTOs`: array(4) / object(31 keys)
- `eventDTOs`: array(21) / object(3 keys)

### typed_splits (`get_activity_typed_splits`, ok)

- `activityId`: integer
- `activityUUID.uuid`: string
- `splits`: array(11) / object(32 keys)

### split_summaries (`get_activity_split_summaries`, ok)

- `activityId`: integer
- `activityUUID.uuid`: string
- `splitSummaries`: array(2) / object(23 keys)

### weather (`get_activity_weather`, ok)

- `issueDate`: string
- `temp`: integer
- `apparentTemp`: integer
- `dewPoint`: integer
- `relativeHumidity`: integer
- `windDirection`: integer
- `windDirectionCompassPoint`: string
- `windSpeed`: integer
- `windGust`: null
- `latitude`: number
- `longitude`: number
- `weatherStationDTO.id`: string
- `weatherStationDTO.name`: string
- `weatherStationDTO.timezone`: null
- `weatherTypeDTO.weatherTypePk`: null
- `weatherTypeDTO.desc`: string
- `weatherTypeDTO.image`: null

### hr_zones (`get_activity_hr_in_timezones`, ok)

- (Wurzel): array(5) / {'zoneNumber': 'integer', 'secsInZone': 'number', 'zoneLowBoundary': 'integer'}

### power_zones_activity (`get_activity_power_in_timezones`, empty)

- (Wurzel): array(0)

### exercise_sets (`get_activity_exercise_sets`, ok)

- `activityId`: integer
- `exerciseSets`: null

### gear_activity (`get_activity_gear`, ok)

- (Wurzel): array(1) / {'gearPk': 'integer', 'uuid': 'string', 'userProfilePk': 'integer', 'gearMakeName': 'string', 'gearModelName': 'string', 'gearTypeName': 'string', 'gearStatusName': 'string', 'displayName': 'string', 'customMakeModel': 'string', 'imageNameLarge': 'null', 'imageNameMedium': 'null', 'imageNameSmall': 'null', 'dateBegin': 'string', 'dateEnd': 'null', 'maximumMeters': 'number', 'notified': 'boolean', 'createDate': 'string', 'updateDate': 'string'}

### original (`download_activity`, ok)

- ``: bytes(149791)

### user_summary (`get_user_summary`, ok)

- ``: object(94 keys)

### steps (`get_steps_data`, ok)

- (Wurzel): array(96) / {'startGMT': 'string', 'endGMT': 'string', 'steps': 'integer', 'pushes': 'integer', 'primaryActivityLevel': 'string', 'activityLevelConstant': 'boolean'}

### floors (`get_floors`, ok)

- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `floorsValueDescriptorDTOList`: array(4) / object(2 keys)
- `floorValuesArray`: array(96) / ['array(4)', 'string']

### heart_rates (`get_heart_rates`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `maxHeartRate`: integer
- `minHeartRate`: integer
- `restingHeartRate`: integer
- `lastSevenDaysAvgRestingHeartRate`: integer
- `heartRateValueDescriptors`: array(2) / object(2 keys)
- `heartRateValues`: array(675) / ['array(2)', 'integer']

### rhr (`get_rhr_day`, ok)

- `userProfileId`: integer
- `statisticsStartDate`: string
- `statisticsEndDate`: string
- `allMetrics.metricsMap`: object(1 keys)
- `groupedMetrics`: null

### stress (`get_stress_data`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `maxStressLevel`: integer
- `avgStressLevel`: integer
- `stressChartValueOffset`: integer
- `stressChartYAxisOrigin`: integer
- `stressValueDescriptorsDTOList`: array(2) / object(2 keys)
- `stressValuesArray`: array(480) / ['array(2)', 'integer']
- `bodyBatteryValueDescriptorsDTOList`: array(4) / object(2 keys)
- `bodyBatteryValuesArray`: array(480) / ['array(4)', 'integer']

### all_day_stress (`get_all_day_stress`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `maxStressLevel`: integer
- `avgStressLevel`: integer
- `stressChartValueOffset`: integer
- `stressChartYAxisOrigin`: integer
- `stressValueDescriptorsDTOList`: array(2) / object(2 keys)
- `stressValuesArray`: array(480) / ['array(2)', 'integer']
- `bodyBatteryValueDescriptorsDTOList`: array(4) / object(2 keys)
- `bodyBatteryValuesArray`: array(480) / ['array(4)', 'integer']

### body_battery_events (`get_body_battery_events`, ok)

- (Wurzel): array(3) / {'event': 'object(7 keys)', 'activityName': 'null', 'activityType': 'null', 'activityId': 'null', 'averageStress': 'number', 'stressValueDescriptorsDTOList': ['array(2)', 'object(2 keys)'], 'stressValuesArray': ['array(144)', ['array(2)', 'integer']], 'bodyBatteryValueDescriptorsDTOList': ['array(4)', 'object(2 keys)'], 'bodyBatteryValuesArray': ['array(144)', ['array(4)', 'integer']]}

### respiration (`get_respiration_data`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `sleepStartTimestampGMT`: string
- `sleepEndTimestampGMT`: string
- `sleepStartTimestampLocal`: string
- `sleepEndTimestampLocal`: string
- `tomorrowSleepStartTimestampGMT`: string
- `tomorrowSleepEndTimestampGMT`: string
- `tomorrowSleepStartTimestampLocal`: string
- `tomorrowSleepEndTimestampLocal`: string
- `lowestRespirationValue`: number
- `highestRespirationValue`: number
- `avgWakingRespirationValue`: number
- `avgSleepRespirationValue`: number
- `avgTomorrowSleepRespirationValue`: number
- `respirationValueDescriptorsDTOList`: array(2) / object(2 keys)
- `respirationValuesArray`: array(720) / ['array(2)', 'integer']
- `respirationAveragesValueDescriptorDTOList`: array(4) / object(2 keys)
- `respirationAveragesValuesArray`: array(24) / ['array(4)', 'integer']
- `respirationVersion`: integer

### spo2 (`get_spo2_data`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `sleepStartTimestampGMT`: string
- `sleepEndTimestampGMT`: string
- `sleepStartTimestampLocal`: string
- `sleepEndTimestampLocal`: string
- `tomorrowSleepStartTimestampGMT`: string
- `tomorrowSleepEndTimestampGMT`: string
- `tomorrowSleepStartTimestampLocal`: string
- `tomorrowSleepEndTimestampLocal`: string
- `averageSpO2`: number
- `lowestSpO2`: integer
- `lastSevenDaysAvgSpO2`: number
- `latestSpO2`: integer
- `latestSpO2TimestampGMT`: string
- `latestSpO2TimestampLocal`: string
- `avgSleepSpO2`: number
- `avgTomorrowSleepSpO2`: number
- `spO2ValueDescriptorsDTOList`: array(3) / object(2 keys)
- `spO2SingleValues`: null
- `continuousReadingDTOList`: null
- `spO2HourlyAverages`: array(10) / ['array(2)', 'integer']

### intensity_minutes (`get_intensity_minutes_data`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `weeklyModerate`: integer
- `weeklyVigorous`: integer
- `weeklyTotal`: integer
- `weekGoal`: integer
- `dayOfGoalMet`: string
- `startDayMinutes`: integer
- `endDayMinutes`: integer
- `moderateMinutes`: integer
- `vigorousMinutes`: integer
- `imValueDescriptorsDTOList`: array(2) / object(2 keys)
- `imValuesArray`: array(4) / ['array(2)', 'integer']

### sleep (`get_sleep_data`, ok)

- `dailySleepDTO.id`: integer
- `dailySleepDTO.userProfilePK`: integer
- `dailySleepDTO.calendarDate`: string
- `dailySleepDTO.sleepTimeSeconds`: integer
- `dailySleepDTO.napTimeSeconds`: integer
- `dailySleepDTO.sleepWindowConfirmed`: boolean
- `dailySleepDTO.sleepWindowConfirmationType`: string
- `dailySleepDTO.sleepStartTimestampGMT`: integer
- `dailySleepDTO.sleepEndTimestampGMT`: integer
- `dailySleepDTO.sleepStartTimestampLocal`: integer
- `dailySleepDTO.sleepEndTimestampLocal`: integer
- `dailySleepDTO.autoSleepStartTimestampGMT`: null
- `dailySleepDTO.autoSleepEndTimestampGMT`: null
- `dailySleepDTO.sleepQualityTypePK`: null
- `dailySleepDTO.sleepResultTypePK`: null
- `dailySleepDTO.unmeasurableSleepSeconds`: integer
- `dailySleepDTO.deepSleepSeconds`: integer
- `dailySleepDTO.lightSleepSeconds`: integer
- `dailySleepDTO.remSleepSeconds`: integer
- `dailySleepDTO.awakeSleepSeconds`: integer
- `dailySleepDTO.deviceRemCapable`: boolean
- `dailySleepDTO.retro`: boolean
- `dailySleepDTO.sleepFromDevice`: boolean
- `dailySleepDTO.averageSpO2Value`: number
- `dailySleepDTO.lowestSpO2Value`: integer
- `dailySleepDTO.highestSpO2Value`: integer
- `dailySleepDTO.averageSpO2HRSleep`: number
- `dailySleepDTO.averageRespirationValue`: number
- `dailySleepDTO.lowestRespirationValue`: number
- `dailySleepDTO.highestRespirationValue`: number
- `dailySleepDTO.awakeCount`: integer
- `dailySleepDTO.avgSleepStress`: number
- `dailySleepDTO.ageGroup`: string
- `dailySleepDTO.avgHeartRate`: number
- `dailySleepDTO.sleepScoreFeedback`: string
- `dailySleepDTO.sleepScoreInsight`: string
- `dailySleepDTO.sleepScorePersonalizedInsight`: string
- `dailySleepDTO.sleepScores`: object(8 keys)
- `dailySleepDTO.sleepVersion`: integer
- `dailySleepDTO.sleepNeed`: object(13 keys)
- `dailySleepDTO.nextSleepNeed`: object(13 keys)
- `sleepMovement`: array(545) / object(3 keys)
- `remSleepData`: boolean
- `sleepLevels`: array(28) / object(3 keys)
- `sleepRestlessMoments`: array(38) / object(2 keys)
- `restlessMomentsCount`: integer
- `wellnessSpO2SleepSummaryDTO.userProfilePk`: integer
- `wellnessSpO2SleepSummaryDTO.deviceId`: integer
- `wellnessSpO2SleepSummaryDTO.sleepMeasurementStartGMT`: string
- `wellnessSpO2SleepSummaryDTO.sleepMeasurementEndGMT`: string
- `wellnessSpO2SleepSummaryDTO.alertThresholdValue`: null
- `wellnessSpO2SleepSummaryDTO.numberOfEventsBelowThreshold`: null
- `wellnessSpO2SleepSummaryDTO.durationOfEventsBelowThreshold`: null
- `wellnessSpO2SleepSummaryDTO.averageSPO2`: number
- `wellnessSpO2SleepSummaryDTO.averageSpO2HR`: number
- `wellnessSpO2SleepSummaryDTO.lowestSPO2`: integer
- `wellnessEpochSPO2DataDTOList`: array(364) / object(7 keys)
- `wellnessEpochRespirationDataDTOList`: array(213) / object(2 keys)
- `wellnessEpochRespirationAveragesList`: array(8) / object(4 keys)
- `respirationVersion`: integer
- `sleepHeartRate`: array(213) / object(2 keys)
- `sleepStress`: array(143) / object(2 keys)
- `sleepBodyBattery`: array(143) / object(2 keys)
- `skinTempDataExists`: boolean
- `hrvData`: array(85) / object(2 keys)
- `avgOvernightHrv`: number
- `hrvStatus`: string
- `bodyBatteryChange`: integer
- `restingHeartRate`: integer

### hrv (`get_hrv_data`, ok)

- `userProfilePk`: integer
- `hrvSummary.calendarDate`: string
- `hrvSummary.weeklyAvg`: integer
- `hrvSummary.lastNightAvg`: integer
- `hrvSummary.lastNight5MinHigh`: integer
- `hrvSummary.baseline`: object(4 keys)
- `hrvSummary.status`: string
- `hrvSummary.feedbackPhrase`: string
- `hrvSummary.createTimeStamp`: string
- `hrvReadings`: array(85) / object(3 keys)
- `startTimestampGMT`: string
- `endTimestampGMT`: string
- `startTimestampLocal`: string
- `endTimestampLocal`: string
- `sleepStartTimestampGMT`: string
- `sleepEndTimestampGMT`: string
- `sleepStartTimestampLocal`: string
- `sleepEndTimestampLocal`: string

### training_readiness (`get_training_readiness`, ok)

- (Wurzel): array(2) / {'userProfilePK': 'integer', 'calendarDate': 'string', 'timestamp': 'string', 'timestampLocal': 'string', 'deviceId': 'integer', 'level': 'string', 'feedbackLong': 'string', 'feedbackShort': 'string', 'score': 'integer', 'sleepScore': 'integer', 'sleepScoreFactorPercent': 'integer', 'sleepScoreFactorFeedback': 'string', 'recoveryTime': 'integer', 'recoveryTimeFactorPercent': 'integer', 'recoveryTimeFactorFeedback': 'string', 'acwrFactorPercent': 'integer', 'acwrFactorFeedback': 'string', 'acuteLoad': 'integer', 'stressHistoryFactorPercent': 'integer', 'stressHistoryFactorFeedback': 'string', 'hrvFactorPercent': 'integer', 'hrvFactorFeedback': 'string', 'hrvWeeklyAverage': 'integer', 'sleepHistoryFactorPercent': 'integer', 'sleepHistoryFactorFeedback': 'string', 'validSleep': 'boolean', 'inputContext': 'string', 'primaryActivityTracker': 'boolean', 'recoveryTimeChangePhrase': 'string'}

### training_status (`get_training_status`, ok)

- `userId`: integer
- `mostRecentVO2Max.userId`: integer
- `mostRecentVO2Max.generic`: object(6 keys)
- `mostRecentVO2Max.cycling`: null
- `mostRecentVO2Max.heatAltitudeAcclimation`: object(16 keys)
- `mostRecentTrainingLoadBalance.userId`: integer
- `mostRecentTrainingLoadBalance.metricsTrainingLoadBalanceDTOMap`: object(1 keys)
- `mostRecentTrainingLoadBalance.recordedDevices`: array(1) / object(4 keys)
- `mostRecentTrainingStatus.userId`: integer
- `mostRecentTrainingStatus.latestTrainingStatusData`: object(1 keys)
- `mostRecentTrainingStatus.recordedDevices`: array(1) / object(4 keys)
- `mostRecentTrainingStatus.showSelector`: boolean
- `mostRecentTrainingStatus.lastPrimarySyncDate`: string
- `heatAltitudeAcclimationDTO`: null

### daily_training_status (`get_daily_training_status`, ok)

- `userId`: integer
- `latestTrainingStatusData.3468557323`: object(17 keys)
- `recordedDevices`: array(1) / object(4 keys)
- `showSelector`: boolean
- `lastPrimarySyncDate`: string

### morning_training_readiness (`get_morning_training_readiness`, ok)

- `userProfilePK`: integer
- `calendarDate`: string
- `timestamp`: string
- `timestampLocal`: string
- `deviceId`: integer
- `level`: string
- `feedbackLong`: string
- `feedbackShort`: string
- `score`: integer
- `sleepScore`: integer
- `sleepScoreFactorPercent`: integer
- `sleepScoreFactorFeedback`: string
- `recoveryTime`: integer
- `recoveryTimeFactorPercent`: integer
- `recoveryTimeFactorFeedback`: string
- `acwrFactorPercent`: integer
- `acwrFactorFeedback`: string
- `acuteLoad`: integer
- `stressHistoryFactorPercent`: integer
- `stressHistoryFactorFeedback`: string
- `hrvFactorPercent`: integer
- `hrvFactorFeedback`: string
- `hrvWeeklyAverage`: integer
- `sleepHistoryFactorPercent`: integer
- `sleepHistoryFactorFeedback`: string
- `validSleep`: boolean
- `inputContext`: string
- `primaryActivityTracker`: boolean
- `recoveryTimeChangePhrase`: string

### fitness_age (`get_fitnessage_data`, ok)

- `chronologicalAge`: integer
- `fitnessAge`: number
- `achievableFitnessAge`: number
- `previousFitnessAge`: number
- `components.vigorousDaysAvg`: object(6 keys)
- `components.rhr`: object(2 keys)
- `components.vigorousMinutesAvg`: object(3 keys)
- `components.bmi`: object(7 keys)
- `lastUpdated`: string

### max_metrics (`get_max_metrics`, empty)

- (Wurzel): array(0)

### four_week_load_balance (`get_training_four_week_load_balance`, ok)

- `userId`: integer
- `metricsTrainingLoadBalanceDTOMap.3468557323`: object(13 keys)
- `recordedDevices`: array(1) / object(4 keys)

### activities_by_date (`get_activities_by_date`, ok)

- (Wurzel): array(4) / object(93 keys)

### body_battery (`get_body_battery`, ok)

- (Wurzel): array(7) / {'date': 'string', 'charged': 'integer', 'drained': 'integer', 'startTimestampGMT': 'string', 'endTimestampGMT': 'string', 'startTimestampLocal': 'string', 'endTimestampLocal': 'string', 'bodyBatteryValuesArray': ['array(6)', ['array(2)', 'integer']], 'bodyBatteryValueDescriptorDTOList': ['array(2)', 'object(2 keys)'], 'bodyBatteryDynamicFeedbackEvent': 'object(4 keys)', 'bodyBatteryActivityEvent': ['array(3)', 'object(7 keys)'], 'endOfDayBodyBatteryDynamicFeedbackEvent': 'object(4 keys)'}

### hrv_range (`get_hrv_data_range`, ok)

- `hrvSummaries`: array(7) / object(8 keys)
- `userProfilePk`: integer

### endurance_score (`get_endurance_score`, ok)

- `userProfilePK`: integer
- `startDate`: string
- `endDate`: string
- `avg`: integer
- `max`: integer
- `groupMap.2026-10-07`: object(3 keys)
- `enduranceScoreDTO.userProfilePK`: integer
- `enduranceScoreDTO.deviceId`: integer
- `enduranceScoreDTO.calendarDate`: string
- `enduranceScoreDTO.overallScore`: integer
- `enduranceScoreDTO.classification`: integer
- `enduranceScoreDTO.feedbackPhrase`: integer
- `enduranceScoreDTO.primaryTrainingDevice`: boolean
- `enduranceScoreDTO.gaugeLowerLimit`: integer
- `enduranceScoreDTO.classificationLowerLimitIntermediate`: integer
- `enduranceScoreDTO.classificationLowerLimitTrained`: integer
- `enduranceScoreDTO.classificationLowerLimitWellTrained`: integer
- `enduranceScoreDTO.classificationLowerLimitExpert`: integer
- `enduranceScoreDTO.classificationLowerLimitSuperior`: integer
- `enduranceScoreDTO.classificationLowerLimitElite`: integer
- `enduranceScoreDTO.gaugeUpperLimit`: integer
- `enduranceScoreDTO.contributors`: array(4) / object(3 keys)

### hill_score (`get_hill_score`, ok)

- `userProfilePK`: integer
- `startDate`: string
- `endDate`: string
- `periodAvgScore.2026-10-07`: integer
- `maxScore`: integer
- `hillScoreDTOList`: array(7) / object(11 keys)

### running_tolerance (`get_running_tolerance`, empty)

- (Wurzel): array(0)

### race_predictions (`get_race_predictions`, error)

Keine Felder erfasst.

### max_metrics_range (`get_max_metrics_range`, ok)

- (Wurzel): array(1) / {'userId': 'integer', 'generic': 'object(6 keys)', 'cycling': 'null', 'heatAltitudeAcclimation': 'object(16 keys)'}

### daily_steps (`get_daily_steps`, ok)

- (Wurzel): array(7) / {'calendarDate': 'string', 'totalSteps': 'integer', 'totalDistance': 'integer', 'stepGoal': 'integer'}

### sleep_daily (`get_sleep_daily`, ok)

- (Wurzel): array(7) / {'calendarDate': 'string', 'values': 'object(22 keys)'}

### rhr_daily (`get_rhr_daily`, ok)

- (Wurzel): array(7) / {'calendarDate': 'string', 'value': 'number'}

### calories_daily (`get_calories_daily`, ok)

- (Wurzel): array(7) / {'calendarDate': 'string', 'active': 'number', 'resting': 'number', 'total': 'number'}

### training_load_activities (`get_training_load_activities`, ok)

- (Wurzel): array(4) / {'activityId': 'integer', 'trainingEffectLabel': 'string', 'trainingEffectLabelSrvrCalc': 'null', 'activityTrainingLoad': 'number', 'startLocal': 'string'}

### weekly_intensity_minutes (`get_weekly_intensity_minutes`, ok)

- (Wurzel): array(2) / {'calendarDate': 'string', 'weeklyGoal': 'integer', 'moderateValue': 'integer', 'vigorousValue': 'integer'}

### weigh_ins (`get_weigh_ins`, ok)

- `dailyWeightSummaries`: array(0)
- `totalAverage.from`: integer
- `totalAverage.until`: integer
- `totalAverage.weight`: null
- `totalAverage.bmi`: null
- `totalAverage.bodyFat`: null
- `totalAverage.bodyWater`: null
- `totalAverage.boneMass`: null
- `totalAverage.muscleMass`: null
- `totalAverage.physiqueRating`: null
- `totalAverage.visceralFat`: null
- `totalAverage.metabolicAge`: null
- `previousDateWeight`: null
- `nextDateWeight`: null

### body_composition (`get_body_composition`, ok)

- `startDate`: string
- `endDate`: string
- `dateWeightList`: array(0)
- `totalAverage.from`: integer
- `totalAverage.until`: integer
- `totalAverage.weight`: null
- `totalAverage.bmi`: null
- `totalAverage.bodyFat`: null
- `totalAverage.bodyWater`: null
- `totalAverage.boneMass`: null
- `totalAverage.muscleMass`: null
- `totalAverage.physiqueRating`: null
- `totalAverage.visceralFat`: null
- `totalAverage.metabolicAge`: null

### blood_pressure (`get_blood_pressure`, ok)

- `from`: string
- `until`: string
- `measurementSummaries`: array(0)
- `categoryStats`: null

### hydration (`get_hydration_data`, ok)

- `userId`: integer
- `calendarDate`: string
- `valueInML`: number
- `goalInML`: number
- `dailyAverageinML`: null
- `lastEntryTimestampLocal`: string
- `sweatLossInML`: number
- `activityIntakeInML`: number

### nutrition_food_log (`get_nutrition_daily_food_log`, ok)

- `mealDate`: string
- `dayStartTime`: string
- `dayEndTime`: string
- `dailyViewType`: string
- `mealDetails`: array(0)
- `loggedFoodsWithServingSizes`: array(0)

### nutrition_meals (`get_nutrition_daily_meals`, ok)

- `meals`: array(0)
- `dailyViewType`: string

### nutrition_settings (`get_nutrition_daily_settings`, empty)

Keine Felder erfasst.

### lifestyle_logging (`get_lifestyle_logging_data`, ok)

- `calendarDate`: string
- `dailyLogsReport`: null
- `completionStats`: array(7) / object(3 keys)

## Folgen

- `bridge/registry.yaml` und `registry.py` tragen Status und `max_span_days` aus dieser Messung.
- `race_predictions` scheitert mit `ValueError` (Aufruf der Bibliothek mit den Bereichsparametern). Muss in der Bibliothek 0.3.17 geprüft werden, bis dahin nicht abrufbar.
- `FIELD_MAP` in `normalize.py` (O9) ist anhand der Feldnamen oben zu prüfen und anzupassen, nicht in diesem Schritt.
