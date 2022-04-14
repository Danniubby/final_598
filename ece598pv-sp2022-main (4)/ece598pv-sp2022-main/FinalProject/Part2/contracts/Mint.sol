// SPDX-License-Identifier: MIT
pragma solidity >=0.8.0 <0.9.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "./interfaces/IPriceFeed.sol";
import "./interfaces/IMint.sol";
import "./sAsset.sol";
import "./EUSD.sol";

contract Mint is Ownable, IMint{

    struct Asset {
        address token;
        uint minCollateralRatio;
        address priceFeed;
    }

    struct Position {
        uint idx;
        address owner;
        uint collateralAmount;
        address assetToken;
        uint assetAmount;
    }

    mapping(address => Asset) _assetMap;
    uint _currentPositionIndex;
    mapping(uint => Position) _idxPositionMap;
    address public collateralToken;

    constructor(address collateral) {
        collateralToken = collateral;
    }

    function registerAsset(address assetToken, uint minCollateralRatio, address priceFeed) external override onlyOwner {
        require(assetToken != address(0), "Invalid assetToken address");
        require(minCollateralRatio >= 1, "minCollateralRatio must be greater than 100%");
        require(_assetMap[assetToken].token == address(0), "Asset was already registered");
        
        _assetMap[assetToken] = Asset(assetToken, minCollateralRatio, priceFeed);
    }

    function getPosition(uint positionIndex) external view returns (address, uint, address, uint) {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        return (position.owner, position.collateralAmount, position.assetToken, position.assetAmount);
    }

    function getMintAmount(uint collateralAmount, address assetToken, uint collateralRatio) public view returns (uint) {
        Asset storage asset = _assetMap[assetToken];
        (int relativeAssetPrice, ) = IPriceFeed(asset.priceFeed).getLatestPrice();
        uint8 decimal = sAsset(assetToken).decimals();
        uint mintAmount = collateralAmount * (10 ** uint256(decimal)) / uint(relativeAssetPrice) / collateralRatio ;
        return mintAmount;
    }

    function checkRegistered(address assetToken) public view returns (bool) {
        return _assetMap[assetToken].token == assetToken;
    }

    function openPosition(uint collateralAmount, address assetToken, uint collateralRatio) external override {
        require(checkRegistered(assetToken), "Asset is not registered");
        require(collateralRatio >= _assetMap[assetToken].minCollateralRatio, "Input collateral ratio is too low");

        // transfer collaterral amount to the contract
        EUSD(collateralToken).transferFrom(msg.sender, address(this), collateralAmount);

        // mint the sender asset
        uint assetAmount = getMintAmount(collateralAmount, assetToken, collateralRatio);
        address assetAddr = _assetMap[assetToken].token;
        sAsset(assetAddr).mint(msg.sender, assetAmount);

        // insert into the position map
        _idxPositionMap[_currentPositionIndex] = Position(_currentPositionIndex, msg.sender, collateralAmount, assetToken, assetAmount);
        _currentPositionIndex += 1;
    }

    function closePosition(uint positionIndex) external override {
        require(msg.sender == _idxPositionMap[positionIndex].owner, "Only the owner can close a position");

        // transfer EUSD tokens locked in the position to the message sender
        EUSD(collateralToken).transfer(msg.sender, _idxPositionMap[positionIndex].collateralAmount);

        // burn all sAssets locked in the position
        sAsset(_idxPositionMap[positionIndex].assetToken).burn(_idxPositionMap[positionIndex].owner, _idxPositionMap[positionIndex].assetAmount);

        // delete the position
        delete _idxPositionMap[positionIndex];
    }

    function deposit(uint positionIndex, uint collateralAmount) external override {
        require(msg.sender == _idxPositionMap[positionIndex].owner, "Only the owner can deposit collateral");

        // transfer collaterral amount to the contract
        EUSD(collateralToken).transferFrom(msg.sender, address(this), collateralAmount);

        // increase collateral amount in the position struct
        _idxPositionMap[positionIndex].collateralAmount += collateralAmount;
    }

    function withdraw(uint positionIndex, uint withdrawAmount) external override {
        require(msg.sender == _idxPositionMap[positionIndex].owner, "Only the owner can withdraw collateral");
        require(_idxPositionMap[positionIndex].collateralAmount-withdrawAmount/_idxPositionMap[positionIndex].assetAmount >= _assetMap[_idxPositionMap[positionIndex].assetToken].minCollateralRatio, "MCR is too low");

        // transfer collaterral amount to the contract
        EUSD(collateralToken).transfer(msg.sender, withdrawAmount);

        // increase collateral amount in the position struct
        _idxPositionMap[positionIndex].collateralAmount -= withdrawAmount;
    }

    function mint(uint positionIndex, uint mintAmount) external override {
        require(msg.sender == _idxPositionMap[positionIndex].owner, "Only the owner can mint tokens");
        require(_idxPositionMap[positionIndex].collateralAmount/(_idxPositionMap[positionIndex].assetAmount+mintAmount) >= _assetMap[_idxPositionMap[positionIndex].assetToken].minCollateralRatio, "MCR is too low");

        sAsset(_idxPositionMap[positionIndex].assetToken).mint(msg.sender, mintAmount);
        _idxPositionMap[positionIndex].assetAmount += mintAmount;
    }

    function burn(uint positionIndex, uint burnAmount) external override {
        require(msg.sender == _idxPositionMap[positionIndex].owner, "Only the owner can burn token");

        sAsset(_idxPositionMap[positionIndex].assetToken).burn(msg.sender, burnAmount);
        _idxPositionMap[positionIndex].assetAmount -= burnAmount;
    }
}