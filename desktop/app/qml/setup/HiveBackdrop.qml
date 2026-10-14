// SPDX-License-Identifier: GPL-3.0-or-later
import QtQuick

// The setup window's backdrop: a honeycomb growing out of `origin`, breathing
// slowly, with a soft pulse of light every few seconds (shaders/hive.frag).
// Still, and fully grown, when animations are turned off in Windows.
ShaderEffect {
    id: hive

    property bool animate: true
    property real cell: 24
    property real strength: 1
    property color lineColor
    property color glowColor
    property point origin: Qt.point(width / 2, height / 3)

    readonly property size itemSize: Qt.size(width, height)
    readonly property real pulse: animate ? 1 : 0
    property real time: 0
    // 0 to 1 as it grows out; the shader gets it in pixels.
    property real grow: animate ? 0 : 1
    readonly property real reveal: grow * Math.hypot(width, height)

    blending: true
    fragmentShader: "qrc:/shaders/hive.frag.qsb"

    NumberAnimation on time {
        running: hive.animate && hive.visible
        from: 0
        to: 3600
        duration: 3600 * 1000
        loops: Animation.Infinite
    }

    // Grows out once, when the window opens.
    NumberAnimation on grow {
        running: hive.animate
        from: 0
        to: 1
        duration: 1800
        easing.type: Easing.OutCubic
    }
}
