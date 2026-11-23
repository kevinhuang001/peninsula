# Capsule freshness recording

[45-second native Linux recording](../docs/media/capsule-refresh-linux.mp4)

The recording runs the actual app, the MPRIS backend and the local LRC lookup.
It plays no audio. It covers pause/resume, forward/backward seek and 2× playback.
Power is an explicitly simulated input because the recording machine has no
physical battery. It covers charging, a charge limit, full charge, unplugging,
unknown state and recovery. It is not a hardware or Windows/macOS validation.
Captions are added below the captured app; the app region is an unmodified screen
recording. The saved MP4 was played back and decoded in full for verification.

## Reproduce

Install `dbus-next` in a Python virtualenv. Use `dbus-run-session` to run the app
and fixture on the same private bus. In that shell:

```sh
cargo build --locked --features demo-power
export PENINSULA_DEMO_POWER_FILE="$PWD/work/demo-power.json"
mkdir -p work
python scripts/demo-capsule.py --power-file "$PENINSULA_DEMO_POWER_FILE" \
  --start-file "$PWD/work/start-recording" &
target/debug/island-lab
```

Create the `work/start-recording` file when the recorder is ready. Use a fresh
file path for another run. The fixture waits at the opening line until then.
It transitions automatically for 45 seconds. The `demo-power` feature and the
explicit environment variable are both required for power injection; normal
builds cannot consume simulated power. Run the fixture and app in an isolated
XDG data/config/state directory when testing alongside another app instance.

## Coverage

- Mailbox replacement ensures a stalled consumer gets the latest observation.
- Power updates invalidate Capsule even when no metrics update arrives; later
  CPU samples cannot overwrite newer power state.
- Windows decoding distinguishes AC connected from the actual charging flag,
  preserves AC state when percentage is unknown, and handles no battery.
- Linux detects multiple adapters and differentiates offline from unknown;
  macOS parses the authoritative power source line.
- Subsecond lyric deadlines cover exact boundaries, pause, playback rate and
  backward seeking. Artwork loaders run off the media observation thread.
- Native power polling uses a separate 250 ms worker. GPU queries run on their
  own worker. CPU/network/process collection targets a one-second cadence.
