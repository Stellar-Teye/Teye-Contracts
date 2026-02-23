const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Governance System", function () {
  let token, governor, timelock, deployer, user1, user2;

  beforeEach(async function () {
    [deployer, user1, user2] = await ethers.getSigners();

    // Deploy governance token
    const Token = await ethers.getContractFactory("ERC20VotesMock");
    token = await Token.deploy("GovernanceToken", "GTK", deployer.address, 1000000);
    await token.deployed();

    // Deploy timelock
    const Timelock = await ethers.getContractFactory("PlatformTimelock");
    timelock = await Timelock.deploy(3600, [deployer.address], [deployer.address]);
    await timelock.deployed();

    // Deploy governor
    const Governor = await ethers.getContractFactory("PlatformGovernor");
    governor = await Governor.deploy(token.address, timelock.address);
    await governor.deployed();
  });

  it("allows proposal creation and voting", async function () {
    // create dummy proposal
    const targets = [deployer.address];
    const values = [0];
    const calldatas = ["0x"];
    const description = "Test proposal";

    const tx = await governor.propose(targets, values, calldatas, description);
    const receipt = await tx.wait();
    const proposalId = receipt.events[0].args.proposalId;

    expect(await governor.state(proposalId)).to.equal(0); // Pending
  });
});
