using Godot;
using System;

public class FPSCamera : Spatial
{
	bool alternate;

    public override void _Ready()
    {
		alternate = false;
    }

	public override void _Process(float delta) {
		var rb = (RigidBody) GetParent();

		var movement = new Vector3();
		var tilt = new Vector2();
		var speed = 1000f; // XXXX FOR NORMAL ZONES: 3000f;
		if(Input.IsActionPressed("camera_right"))
			movement.x += 1;
		if(Input.IsActionPressed("camera_left"))
			movement.x -= 1;
		if(Input.IsActionPressed("camera_forward"))
			movement.z -= 1;
		if(Input.IsActionPressed("camera_backward"))
			movement.z += 1;
		if(Input.IsActionPressed("camera_fly_up"))
			movement.y += 1;
		if(Input.IsActionPressed("camera_fly_down"))
			movement.y -= 1;
		if(Input.IsActionPressed("camera_look_up"))
			tilt.y += 1;
		if(Input.IsActionPressed("camera_look_down"))
			tilt.y -= 1;
		if(Input.IsActionPressed("camera_look_right"))
			tilt.x += 1;
		if(Input.IsActionPressed("camera_look_left"))
			tilt.x -= 1;
		if(Input.IsActionPressed("turbo"))
			speed *= 20;
		var child = (Spatial) GetChild(0);
		if(movement.length() != 0 && alternate)
			rb.LinearVelocity = Transform.xform(movement * delta * speed) + new Vector3(0, Math.Min(0, rb.LinearVelocity.y), 0);
		alternate = !alternate;
		if(tilt.y != 0)
			child.RotateX(tilt.y * delta);
		if(tilt.x != 0)
			RotateY(-tilt.x * delta * 2);
	}
}
