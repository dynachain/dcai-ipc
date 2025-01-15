// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.19;

library SelectorLibrary {
    function resolveSelectors(string memory facetName) public pure returns (bytes4[] memory facetSelectors) {
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("OwnershipFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000028da5cb5b00000000000000000000000000000000000000000000000000000000f2fde38b00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("DiamondCutFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000011f931c1c00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("DiamondLoupeFacet"))) {
            return
                abi.decode(
                    hex"00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000005cdffacc60000000000000000000000000000000000000000000000000000000052ef6b2c00000000000000000000000000000000000000000000000000000000adfca15e000000000000000000000000000000000000000000000000000000007a0ed6270000000000000000000000000000000000000000000000000000000001ffc9a700000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("GatewayGetterFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000228789f83b0000000000000000000000000000000000000000000000000000000006c46853000000000000000000000000000000000000000000000000000000002da5794a00000000000000000000000000000000000000000000000000000000dd81b5cf0000000000000000000000000000000000000000000000000000000041b6a2e80000000000000000000000000000000000000000000000000000000038d6693200000000000000000000000000000000000000000000000000000000b3ab3f7400000000000000000000000000000000000000000000000000000000ac12d763000000000000000000000000000000000000000000000000000000004aa8f8a500000000000000000000000000000000000000000000000000000000ca41d5ce00000000000000000000000000000000000000000000000000000000444ead5100000000000000000000000000000000000000000000000000000000d6c5c39700000000000000000000000000000000000000000000000000000000544dddff000000000000000000000000000000000000000000000000000000006ad21bb000000000000000000000000000000000000000000000000000000000a517218f000000000000000000000000000000000000000000000000000000009704276600000000000000000000000000000000000000000000000000000000b1ba49b000000000000000000000000000000000000000000000000000000000f3229131000000000000000000000000000000000000000000000000000000000338150f0000000000000000000000000000000000000000000000000000000094074b03000000000000000000000000000000000000000000000000000000007edeac920000000000000000000000000000000000000000000000000000000006572c1a00000000000000000000000000000000000000000000000000000000c66c66a1000000000000000000000000000000000000000000000000000000003594c3c1000000000000000000000000000000000000000000000000000000009d3070b50000000000000000000000000000000000000000000000000000000042398a9a00000000000000000000000000000000000000000000000000000000fa34a400000000000000000000000000000000000000000000000000000000005d02968500000000000000000000000000000000000000000000000000000000599c7bd10000000000000000000000000000000000000000000000000000000005aff0b3000000000000000000000000000000000000000000000000000000008cfd78e7000000000000000000000000000000000000000000000000000000007474d79f0000000000000000000000000000000000000000000000000000000002e30f9a00000000000000000000000000000000000000000000000000000000a2b6715800000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("GatewayManagerFacet"))) {
            return
                abi.decode(
                    hex"00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000007eb4f16b50000000000000000000000000000000000000000000000000000000018f44b70000000000000000000000000000000000000000000000000000000000517e1aa0000000000000000000000000000000000000000000000000000000041c0e1b500000000000000000000000000000000000000000000000000000000d66d6c10000000000000000000000000000000000000000000000000000000006b2c1eef0000000000000000000000000000000000000000000000000000000045f5448500000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("GatewayMessengerFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000027f7999f4000000000000000000000000000000000000000000000000000000002c85ec2c00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("CheckpointingFacet"))) {
            return
                abi.decode(
                    hex"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000453b4e7bf000000000000000000000000000000000000000000000000000000009db11d8c000000000000000000000000000000000000000000000000000000006326379f00000000000000000000000000000000000000000000000000000000ac81837900000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("TopDownFinalityFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000040df144610000000000000000000000000000000000000000000000000000000011196974000000000000000000000000000000000000000000000000000000008fbe0b7c00000000000000000000000000000000000000000000000000000000e49a547d00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("XnetMessagingFacet"))) {
            return
                abi.decode(
                    hex"00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001007cf1ec00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorGetterFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000223354c3e10000000000000000000000000000000000000000000000000000000035142c8c0000000000000000000000000000000000000000000000000000000006c46853000000000000000000000000000000000000000000000000000000004b27aa72000000000000000000000000000000000000000000000000000000004b0694e200000000000000000000000000000000000000000000000000000000b6797d3c000000000000000000000000000000000000000000000000000000008ef3f761000000000000000000000000000000000000000000000000000000006b84e38300000000000000000000000000000000000000000000000000000000903e693000000000000000000000000000000000000000000000000000000000948628a900000000000000000000000000000000000000000000000000000000d92e8f12000000000000000000000000000000000000000000000000000000009de7025800000000000000000000000000000000000000000000000000000000c7cda762000000000000000000000000000000000000000000000000000000009754b29e0000000000000000000000000000000000000000000000000000000038a210b30000000000000000000000000000000000000000000000000000000080f76021000000000000000000000000000000000000000000000000000000005dd9147c00000000000000000000000000000000000000000000000000000000d6eb591000000000000000000000000000000000000000000000000000000000332a5ac9000000000000000000000000000000000000000000000000000000001597bf7e0000000000000000000000000000000000000000000000000000000052d182d1000000000000000000000000000000000000000000000000000000001904bb2e000000000000000000000000000000000000000000000000000000006ad04c7900000000000000000000000000000000000000000000000000000000cfca28240000000000000000000000000000000000000000000000000000000040550a1c00000000000000000000000000000000000000000000000000000000d081be03000000000000000000000000000000000000000000000000000000001f3a0e410000000000000000000000000000000000000000000000000000000072d0a0e000000000000000000000000000000000000000000000000000000000599c7bd1000000000000000000000000000000000000000000000000000000009e33bd0200000000000000000000000000000000000000000000000000000000c5ab224100000000000000000000000000000000000000000000000000000000f0cf6c9600000000000000000000000000000000000000000000000000000000ad81e4d60000000000000000000000000000000000000000000000000000000080875df700000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorManagerFacet"))) {
            return
                abi.decode(
                    hex"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000a10fd4261000000000000000000000000000000000000000000000000000000003ae247130000000000000000000000000000000000000000000000000000000041c0e1b500000000000000000000000000000000000000000000000000000000d66d9e19000000000000000000000000000000000000000000000000000000004d9013a10000000000000000000000000000000000000000000000000000000066783c9b00000000000000000000000000000000000000000000000000000000da5d09ee00000000000000000000000000000000000000000000000000000000dcda897300000000000000000000000000000000000000000000000000000000a694fc3a000000000000000000000000000000000000000000000000000000002e17de7800000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorPauseFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000038456cb59000000000000000000000000000000000000000000000000000000005c975abb000000000000000000000000000000000000000000000000000000003f4ba83a00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorRewardFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000014e71d92d00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorCheckpointingFacet"))) {
            return
                abi.decode(
                    hex"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000025681656700000000000000000000000000000000000000000000000000000000cc2dc2b900000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("RegisterSubnetFacet"))) {
            return
                abi.decode(
                    hex"00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001611941f900000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetGetterFacet"))) {
            return
                abi.decode(
                    hex"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000f42bf3cc10000000000000000000000000000000000000000000000000000000062c9d7fb00000000000000000000000000000000000000000000000000000000967ba537000000000000000000000000000000000000000000000000000000000be06111000000000000000000000000000000000000000000000000000000001b0766c300000000000000000000000000000000000000000000000000000000a372bf30000000000000000000000000000000000000000000000000000000000f5849d1000000000000000000000000000000000000000000000000000000004d7115140000000000000000000000000000000000000000000000000000000089bba29900000000000000000000000000000000000000000000000000000000540b5ad60000000000000000000000000000000000000000000000000000000054a4eddb000000000000000000000000000000000000000000000000000000009836b75f00000000000000000000000000000000000000000000000000000000030f6051000000000000000000000000000000000000000000000000000000001163dca500000000000000000000000000000000000000000000000000000000a46d044d00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorMock"))) {
            return
                abi.decode(
                    hex"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d10fd4261000000000000000000000000000000000000000000000000000000004e71d92d00000000000000000000000000000000000000000000000000000000350a14bf00000000000000000000000000000000000000000000000000000000c7ebdaef000000000000000000000000000000000000000000000000000000003ae247130000000000000000000000000000000000000000000000000000000041c0e1b500000000000000000000000000000000000000000000000000000000d66d9e19000000000000000000000000000000000000000000000000000000004d9013a10000000000000000000000000000000000000000000000000000000066783c9b00000000000000000000000000000000000000000000000000000000da5d09ee00000000000000000000000000000000000000000000000000000000dcda897300000000000000000000000000000000000000000000000000000000a694fc3a000000000000000000000000000000000000000000000000000000002e17de7800000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        if (keccak256(abi.encodePacked(facetName)) == keccak256(abi.encodePacked("SubnetActorActivityFacet"))) {
            return
                abi.decode(
                    hex"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000202eca6eb00000000000000000000000000000000000000000000000000000000f9d3434c00000000000000000000000000000000000000000000000000000000",
                    (bytes4[])
                );
        }
        revert(string.concat("Selectors not found for facet: ", facetName));
    }
}
