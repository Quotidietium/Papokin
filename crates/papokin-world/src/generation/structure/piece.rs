#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructurePieceType {
    // 废弃矿井
    MineshaftCorridor,
    MineshaftCrossing,
    MineshaftRoom,
    MineshaftStairs,

    // 下界要塞
    NetherFortressBridgeCrossing,
    NetherFortressBridgeEnd,
    NetherFortressBridge,
    NetherFortressCorridorStairs,
    NetherFortressCorridorBalcony,
    NetherFortressCorridorExit,
    NetherFortressCorridorCrossing,
    NetherFortressCorridorLeftTurn,
    NetherFortressSmallCorridor,
    NetherFortressCorridorRightTurn,
    NetherFortressCorridorNetherWartsRoom,
    NetherFortressBridgePlatform,
    NetherFortressBridgeSmallCrossing,
    NetherFortressBridgeStairs,
    NetherFortressStart,

    // 要塞
    StrongholdChestCorridor,
    StrongholdSmallCorridor,
    StrongholdFiveWayCrossing,
    StrongholdLeftTurn,
    StrongholdLibrary,
    StrongholdPortalRoom,
    StrongholdPrisonHall,
    StrongholdRightTurn,
    StrongholdSquareRoom,
    StrongholdSpiralStaircase,
    StrongholdStart,
    StrongholdCorridor,
    StrongholdStairs,

    // 主世界/通用
    JungleTemple,
    OceanTemple,
    Igloo,
    RuinedPortal,
    SwampHut,
    DesertTemple,

    // 海底神殿
    OceanMonumentBase,
    OceanMonumentCoreRoom,
    OceanMonumentDoubleXRoom,
    OceanMonumentDoubleXYRoom,
    OceanMonumentDoubleYRoom,
    OceanMonumentDoubleYZRoom,
    OceanMonumentDoubleZRoom,
    OceanMonumentEntryRoom,
    OceanMonumentPenthouse,
    OceanMonumentSimpleRoom,
    OceanMonumentSimpleTopRoom,
    OceanMonumentWingRoom,

    // 末地 / 其他
    EndCity,
    WoodlandMansion,
    BuriedTreasure,
    Shipwreck,
    NetherFossil,
    Jigsaw,
}
