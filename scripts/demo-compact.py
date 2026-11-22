#!/usr/bin/env python3
"""Deterministic real MPRIS/LRC recording scenario with explicitly simulated power.

Requires dbus-next. Launch on a private D-Bus session together with a build using
--features demo-power and PENINSULA_DEMO_POWER_FILE pointing to --power-file.
No audio is played. Production builds have no simulated power input.
"""
import argparse
import asyncio
import json
from pathlib import Path
import time
from dbus_next import Variant, PropertyAccess
from dbus_next.aio import MessageBus
from dbus_next.service import ServiceInterface, dbus_property, method

class Player(ServiceInterface):
    def __init__(self, start):
        super().__init__('org.mpris.MediaPlayer2.Player')
        self.start = start
    def elapsed(self):
        return max(0, time.monotonic() - self.start)
    def timeline(self):
        t = self.elapsed()
        if t < 6: return t, True, 1.0
        if t < 9: return 6.0, False, 1.0
        if t < 14: return t - 3, True, 1.0
        if t < 20: return 16 + t - 14, True, 1.0
        if t < 26: return 2 + t - 20, True, 1.0
        if t < 32: return 8 + (t - 26) * 2, True, 2.0
        return min(26, 20 + t - 32), t < 38, 1.0
    @dbus_property(access=PropertyAccess.READ)
    def PlaybackStatus(self) -> 's':
        return 'Playing' if self.timeline()[1] else 'Paused'
    @dbus_property(access=PropertyAccess.READ)
    def Metadata(self) -> 'a{sv}':
        song = Path(__file__).resolve().parent / 'fixtures/compact-sync-demo.ogg'
        return {'mpris:trackid': Variant('o', '/peninsula/compactdemo'),
                'mpris:length': Variant('x', 40_000_000),
                'xesam:title': Variant('s', 'Compact 实时同步演示'),
                'xesam:artist': Variant('as', ['本地 MPRIS 测试播放器']),
                'xesam:url': Variant('s', song.as_uri())}
    @dbus_property(access=PropertyAccess.READ)
    def Position(self) -> 'x': return int(self.timeline()[0] * 1_000_000)
    @dbus_property(access=PropertyAccess.READ)
    def Rate(self) -> 'd': return self.timeline()[2]
    @dbus_property(access=PropertyAccess.READ)
    def CanControl(self) -> 'b': return False

async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--power-file', type=Path, required=True)
    parser.add_argument('--start-delay', type=float, default=10)
    parser.add_argument('--start-file', type=Path)
    args = parser.parse_args()
    start = time.monotonic() + (1e9 if args.start_file else args.start_delay)
    player = Player(start)
    bus = await MessageBus().connect()
    bus.export('/org/mpris/MediaPlayer2', player)
    await bus.request_name('org.mpris.MediaPlayer2.PeninsulaDemo')
    print('MPRIS ready; power input is SIMULATED.', flush=True)
    previous = None
    while True:
        if args.start_file and args.start_file.exists():
            start = player.start = time.monotonic()
            args.start_file = None
            print('Scenario started.', flush=True)
        t = max(0, time.monotonic() - start)
        if t < 4: state = dict(percent=60, external=False, charging=False)
        elif t < 10: state = dict(percent=60, external=True, charging=True)
        elif t < 17: state = dict(percent=80, external=True, charging=False)
        elif t < 23: state = dict(percent=100, external=True, charging=False)
        elif t < 30: state = dict(percent=99, external=False, charging=False)
        elif t < 37: state = dict(percent=99, external=True, charging=True)
        elif t < 41: state = dict(percent=None, external=None, charging=False)
        else: state = dict(percent=98, external=False, charging=False)
        temp = args.power_file.with_suffix('.tmp')
        temp.write_text(json.dumps(state))
        temp.replace(args.power_file)
        status = (player.PlaybackStatus, player.Rate)
        if status != previous:
            player.emit_properties_changed({'PlaybackStatus': status[0], 'Rate': status[1]})
            previous = status
        await asyncio.sleep(.05)

if __name__ == '__main__': asyncio.run(main())
