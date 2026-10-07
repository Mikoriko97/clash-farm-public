extends Node
## Test double of the `Audio` autoload: remembers which `play_fx` names were asked for (door sounds of the house flow, the bell of
## the city buildings). Add it to `root` under the name "Audio".

var played: Array[String] = []


func play_fx(fx: String) -> bool:
	played.append(fx)
	return true
