// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.19;

import "modules/utils/BytesLib.sol";

import "./VerifyStructs.sol";

contract VerifyMessages is VerifyStructs {
    using BytesLib for bytes;

    /**
     * @notice Encodes the VerifyMessage struct into bytes
     * @param parsedMessage VerifyMessage struct with arbitrary Verify message
     * @return encodedMessage VerifyMessage encoded into bytes
     */
    function encodeMessage(
        VerifyMessage memory parsedMessage
    ) public pure returns (bytes memory encodedMessage) {
        // Convert message string to bytes so that we can use the .length attribute.
        // The length of the arbitrary messages needs to be encoded in the message
        // so that the corresponding decode function can decode the message properly.
        encodedMessage = abi.encodePacked(
            parsedMessage.account,
            parsedMessage.nft,
            parsedMessage.id
        );
    }
}
