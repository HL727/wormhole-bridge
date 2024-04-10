// SPDX-License-Identifier: Apache 2
pragma solidity ^0.8.19;

import "modules/wormhole/IWormhole.sol";
import "modules/utils/BytesLib.sol";

import "./VerifyGetters.sol";
import "./VerifyMessages.sol";

interface IERC721 {
    function ownerOf(uint256 tokenId) external view returns (address);
}

/**
 * @title A Cross-Chain Verify Application
 * @notice This contract uses Wormhole's generic-messaging to send an arbitrary
 * Verify message to registered emitters on foreign blockchains
 */
contract Verify is VerifyGetters, VerifyMessages {
    using BytesLib for bytes;

    address public nft1;
    address public nft2;

    /**
     * @notice Deploys the smart contract and sanity checks initial deployment values
     * @dev Sets the owner, wormhole, chainId and wormholeFinality state variables.
     * See VerifyState.sol for descriptions of each state variable.
     */
    constructor(
        address _nft1,
        address _nft2,
        address wormhole_,
        uint16 chainId_,
        uint8 wormholeFinality_
    ) {
        nft1 = _nft1;
        nft2 = _nft2;
        // sanity check input values
        require(wormhole_ != address(0), "invalid Wormhole address");
        require(chainId_ > 0, "invalid chainId");
        require(wormholeFinality_ > 0, "invalid wormholeFinality");

        // set constructor state values
        setOwner(msg.sender);
        setWormhole(wormhole_);
        setChainId(chainId_);
        setWormholeFinality(wormholeFinality_);
    }

    /**
     * @notice Creates an arbitrary Verify message to be attested by the
     * Wormhole guardians.
     * @dev batchID is set to 0 to opt out of batching in future Wormhole versions.
     * Reverts if:
     * - caller doesn't pass enough value to pay the Wormhole network fee
     * - `VerifyMessage` length is >= max(uint16)
     * @param account Arbitrary Verify string
     * @param nft Arbitrary Verify string
     * @param id Arbitrary Verify string
     * @return messageSequence Wormhole message sequence for this contract
     */
    function verify(
        bytes32 account,
        address nft,
        uint16 id
    ) public payable returns (uint64 messageSequence) {
        require(nft == nft1 || nft == nft2, "shit nft");
        require(IERC721(nft).ownerOf(id) == msg.sender, "not owner");
        // cache Wormhole instance and fees to save on gas
        IWormhole wormhole = wormhole();
        uint256 wormholeFee = wormhole.messageFee();

        // Confirm that the caller has sent enough value to pay for the Wormhole
        // message fee.
        require(msg.value == wormholeFee, "insufficient value");

        // create the VerifyMessage struct
        VerifyMessage memory parsedMessage = VerifyMessage({
            account: account,
            nft: nft,
            id: id
        });

        // encode the VerifyMessage struct into bytes
        bytes memory encodedMessage = encodeMessage(parsedMessage);

        // Send the Verify message by calling publishMessage on the
        // Wormhole core contract and paying the Wormhole protocol fee.
        messageSequence = wormhole.publishMessage{value: wormholeFee}(
            0, // batchID
            encodedMessage,
            wormholeFinality()
        );
    }

    modifier onlyOwner() {
        require(owner() == msg.sender, "caller not the owner");
        _;
    }
}
