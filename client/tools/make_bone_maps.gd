extends SceneTree
## Builds the two BoneMap resources that let the importer retarget every clip
## onto the humanoid profile (docs/ARCHITECTURE.md §Godot): the pack rig ("B-*")
## of HumanM_Model and its original clips, and the Mixamo rig ("mixamorig_*") of
## downloaded clips. Referenced from the *.fbx.import files (`retarget/bone_map`).
## Run: godot --headless --path client --script res://tools/make_bone_maps.gd

const OUT_RIG := "res://assets/character/bone_map_rig.tres"
const OUT_MIXAMO := "res://assets/character/bone_map_mixamo.tres"
const OUT_UAL := "res://assets/character/bone_map_ual.tres"

## Humanoid profile bone → pack rig bone. Sides are expanded below.
const RIG_CENTER := {
	"Root": "B-root", "Hips": "B-hips", "Spine": "B-spine", "Chest": "B-chest",
	"Neck": "B-neck", "Head": "B-head", "Jaw": "B-jaw",
}
const RIG_SIDE := {
	"Shoulder": "B-shoulder.%s", "UpperArm": "B-upperArm.%s", "LowerArm": "B-forearm.%s", "Hand": "B-hand.%s",
	"ThumbMetacarpal": "B-thumb01.%s", "ThumbProximal": "B-thumb02.%s", "ThumbDistal": "B-thumb03.%s",
	"IndexProximal": "B-indexFinger01.%s", "IndexIntermediate": "B-indexFinger02.%s", "IndexDistal": "B-indexFinger03.%s",
	"MiddleProximal": "B-middleFinger01.%s", "MiddleIntermediate": "B-middleFinger02.%s", "MiddleDistal": "B-middleFinger03.%s",
	"RingProximal": "B-ringFinger01.%s", "RingIntermediate": "B-ringFinger02.%s", "RingDistal": "B-ringFinger03.%s",
	"LittleProximal": "B-pinky01.%s", "LittleIntermediate": "B-pinky02.%s", "LittleDistal": "B-pinky03.%s",
	"UpperLeg": "B-thigh.%s", "LowerLeg": "B-shin.%s", "Foot": "B-foot.%s", "Toes": "B-toe.%s",
}
const MIXAMO_CENTER := {
	"Hips": "mixamorig_Hips", "Spine": "mixamorig_Spine", "Chest": "mixamorig_Spine1", "UpperChest": "mixamorig_Spine2",
	"Neck": "mixamorig_Neck", "Head": "mixamorig_Head",
}
const MIXAMO_SIDE := {
	"Shoulder": "mixamorig_%sShoulder", "UpperArm": "mixamorig_%sArm", "LowerArm": "mixamorig_%sForeArm", "Hand": "mixamorig_%sHand",
	"ThumbMetacarpal": "mixamorig_%sHandThumb1", "ThumbProximal": "mixamorig_%sHandThumb2", "ThumbDistal": "mixamorig_%sHandThumb3",
	"IndexProximal": "mixamorig_%sHandIndex1", "IndexIntermediate": "mixamorig_%sHandIndex2", "IndexDistal": "mixamorig_%sHandIndex3",
	"MiddleProximal": "mixamorig_%sHandMiddle1", "MiddleIntermediate": "mixamorig_%sHandMiddle2", "MiddleDistal": "mixamorig_%sHandMiddle3",
	"RingProximal": "mixamorig_%sHandRing1", "RingIntermediate": "mixamorig_%sHandRing2", "RingDistal": "mixamorig_%sHandRing3",
	"LittleProximal": "mixamorig_%sHandPinky1", "LittleIntermediate": "mixamorig_%sHandPinky2", "LittleDistal": "mixamorig_%sHandPinky3",
	"UpperLeg": "mixamorig_%sUpLeg", "LowerLeg": "mixamorig_%sLeg", "Foot": "mixamorig_%sFoot", "Toes": "mixamorig_%sToeBase",
}
## Quaternius Universal Animation Library (UE5 mannequin names, `_l`/`_r`).
const UAL_CENTER := {
	"Root": "root", "Hips": "pelvis", "Spine": "spine_01", "Chest": "spine_02", "UpperChest": "spine_03",
	"Neck": "neck_01", "Head": "Head",
}
const UAL_SIDE := {
	"Shoulder": "clavicle_%s", "UpperArm": "upperarm_%s", "LowerArm": "lowerarm_%s", "Hand": "hand_%s",
	"ThumbMetacarpal": "thumb_01_%s", "ThumbProximal": "thumb_02_%s", "ThumbDistal": "thumb_03_%s",
	"IndexProximal": "index_01_%s", "IndexIntermediate": "index_02_%s", "IndexDistal": "index_03_%s",
	"MiddleProximal": "middle_01_%s", "MiddleIntermediate": "middle_02_%s", "MiddleDistal": "middle_03_%s",
	"RingProximal": "ring_01_%s", "RingIntermediate": "ring_02_%s", "RingDistal": "ring_03_%s",
	"LittleProximal": "pinky_01_%s", "LittleIntermediate": "pinky_02_%s", "LittleDistal": "pinky_03_%s",
	"UpperLeg": "thigh_%s", "LowerLeg": "calf_%s", "Foot": "foot_%s", "Toes": "ball_%s",
}


func _initialize() -> void:
	_save(_build(RIG_CENTER, RIG_SIDE, {"Left": "L", "Right": "R"}), OUT_RIG)
	_save(_build(MIXAMO_CENTER, MIXAMO_SIDE, {"Left": "Left", "Right": "Right"}), OUT_MIXAMO)
	_save(_build(UAL_CENTER, UAL_SIDE, {"Left": "l", "Right": "r"}), OUT_UAL)
	quit()


func _build(center: Dictionary, side: Dictionary, suffixes: Dictionary) -> BoneMap:
	var map := BoneMap.new()
	map.profile = SkeletonProfileHumanoid.new()
	var profile := map.profile
	var known: Array[String] = []
	for i: int in profile.bone_size:
		known.append(String(profile.get_bone_name(i)))
	var set_count := 0
	for profile_bone: String in center:
		if profile_bone in known:
			map.set_skeleton_bone_name(profile_bone, center[profile_bone])
			set_count += 1
	for prefix: String in suffixes:
		for part: String in side:
			var profile_bone := prefix + part
			if profile_bone in known:
				map.set_skeleton_bone_name(profile_bone, (side[part] as String) % suffixes[prefix])
				set_count += 1
	print("bone map: %d / %d profile bones mapped" % [set_count, profile.bone_size])
	return map


func _save(map: BoneMap, path: String) -> void:
	var err := ResourceSaver.save(map, path)
	print("%s: %s" % [path, error_string(err)])
