import React, { useState, useEffect } from 'react';
import { Wallet, TrendingUp, Send, ArrowDownToLine, Shield, Clock, AlertCircle } from 'lucide-react';

const BSVBankApp = () => {
  const [connected, setConnected] = useState(false);
  const [paymail, setPaymail] = useState('');
  const [balance, setBalance] = useState(null);
  const [activeTab, setActiveTab] = useState('deposit');
  const [depositAmount, setDepositAmount] = useState('');
  const [lockDays, setLockDays] = useState(30);
  const [withdrawalAddress, setWithdrawalAddress] = useState('');
  const [transactions, setTransactions] = useState([]);
  const [loading, setLoading] = useState(false);

  // Connect to HandCash wallet
  const connectWallet = async () => {
    setLoading(true);
    try {
      // In production: integrate HandCash Connect SDK
      // const handCash = new HandCashConnect({ appId: 'your-app-id' });
      // const account = await handCash.getAccount();
      
      // Simulated connection
      setTimeout(() => {
        const mockPaymail = 'user@handcash.io';
        setPaymail(mockPaymail);
        setConnected(true);
        fetchBalance(mockPaymail);
        setLoading(false);
      }, 1000);
    } catch (error) {
      console.error('Wallet connection failed:', error);
      setLoading(false);
    }
  };

  // Fetch user balance from deposit service
  const fetchBalance = async (userPaymail) => {
    try {
      const response = await fetch(`http://localhost:8080/balance/${encodeURIComponent(userPaymail)}`);
      if (response.ok) {
        const data = await response.json();
        setBalance(data);
      }
    } catch (error) {
      console.error('Failed to fetch balance:', error);
      // Mock data for demo
      setBalance({
        balance_satoshis: 1500000,
        accrued_interest_satoshis: 45000,
        total_available_satoshis: 1545000,
        current_apy: 8.5,
        active_deposits: 3
      });
    }
  };

  // Create deposit
  const handleDeposit = async () => {
    if (!depositAmount || parseFloat(depositAmount) <= 0) {
      alert('Please enter a valid deposit amount');
      return;
    }

    setLoading(true);
    try {
      const satoshis = Math.floor(parseFloat(depositAmount) * 100000000);
      
      // In production: create real BSV transaction via HandCash
      // const payment = await handCashAccount.pay({
      //   to: 'bank-deposit-address',
      //   amount: depositAmount,
      //   currencyCode: 'BSV'
      // });
      
      // Mock transaction ID
      const mockTxid = Array(64).fill(0).map(() => 
        Math.floor(Math.random() * 16).toString(16)
      ).join('');
      
      const response = await fetch('http://localhost:8080/deposits', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          user_paymail: paymail,
          amount_satoshis: satoshis,
          txid: mockTxid,
          lock_duration_days: lockDays > 0 ? lockDays : null
        })
      });

      if (response.ok) {
        const result = await response.json();
        alert(`Deposit created! ID: ${result.deposit_id}\nStatus: ${result.status}`);
        setDepositAmount('');
        fetchBalance(paymail);
        
        // Add to transaction history
        setTransactions(prev => [{
          type: 'deposit',
          amount: satoshis,
          status: result.status,
          timestamp: new Date(),
          id: result.deposit_id
        }, ...prev]);
      }
    } catch (error) {
      console.error('Deposit failed:', error);
      alert('Deposit failed. Please try again.');
    }
    setLoading(false);
  };

  // Initiate withdrawal
  const handleWithdraw = async (depositId) => {
    if (!withdrawalAddress) {
      alert('Please enter a withdrawal address');
      return;
    }

    setLoading(true);
    try {
      // Mock signature (in production: sign with user's private key)
      const mockSignature = Array(128).fill(0).map(() => 
        Math.floor(Math.random() * 16).toString(16)
      ).join('');

      const response = await fetch('http://localhost:8080/withdrawals', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          deposit_id: depositId,
          destination_address: withdrawalAddress,
          signature: mockSignature
        })
      });

      if (response.ok) {
        const result = await response.json();
        alert(`Withdrawal successful!\nTXID: ${result.withdrawal_txid}`);
        setWithdrawalAddress('');
        fetchBalance(paymail);
      } else {
        const error = await response.json();
        alert(`Withdrawal failed: ${error.error}`);
      }
    } catch (error) {
      console.error('Withdrawal failed:', error);
      alert('Withdrawal failed. Please try again.');
    }
    setLoading(false);
  };

  const formatBSV = (satoshis) => {
    return (satoshis / 100000000).toFixed(8);
  };

  const formatUSD = (satoshis, bsvPrice = 50) => {
    return ((satoshis / 100000000) * bsvPrice).toFixed(2);
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-orange-50 via-amber-50 to-yellow-50 p-4">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="bg-white rounded-2xl shadow-xl p-6 mb-6">
          <div className="flex justify-between items-center">
            <div className="flex items-center gap-4">
              <div className="bg-gradient-to-br from-orange-500 to-red-500 p-3 rounded-xl">
                <Wallet className="w-8 h-8 text-white" />
              </div>
              <div>
                <h1 className="text-3xl font-bold text-gray-900">BSV Bank</h1>
                <p className="text-gray-600">Algorithmic Banking on Bitcoin SV</p>
              </div>
            </div>
            
            {!connected ? (
              <button
                onClick={connectWallet}
                disabled={loading}
                className="bg-orange-500 hover:bg-orange-600 text-white px-6 py-3 rounded-xl font-bold transition-all shadow-lg disabled:opacity-50"
              >
                {loading ? 'Connecting...' : 'Connect Wallet'}
              </button>
            ) : (
              <div className="text-right">
                <div className="text-sm text-gray-600 mb-1">Connected</div>
                <div className="font-mono text-sm bg-gray-100 px-3 py-1 rounded-lg">
                  {paymail}
                </div>
              </div>
            )}
          </div>
        </div>

        {connected && balance && (
          <>
            {/* Balance Overview */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
              <div className="bg-gradient-to-br from-blue-500 to-blue-600 rounded-xl p-6 text-white">
                <div className="text-sm opacity-90 mb-2">Total Balance</div>
                <div className="text-2xl font-bold mb-1">
                  {formatBSV(balance.balance_satoshis)} BSV
                </div>
                <div className="text-sm opacity-75">
                  ≈ ${formatUSD(balance.balance_satoshis)} USD
                </div>
              </div>

              <div className="bg-gradient-to-br from-green-500 to-green-600 rounded-xl p-6 text-white">
                <div className="text-sm opacity-90 mb-2">Accrued Interest</div>
                <div className="text-2xl font-bold mb-1">
                  {formatBSV(balance.accrued_interest_satoshis)} BSV
                </div>
                <div className="text-sm opacity-75">
                  ≈ ${formatUSD(balance.accrued_interest_satoshis)} USD
                </div>
              </div>

              <div className="bg-gradient-to-br from-purple-500 to-purple-600 rounded-xl p-6 text-white">
                <div className="text-sm opacity-90 mb-2">Current APY</div>
                <div className="text-2xl font-bold mb-1">
                  {balance.current_apy.toFixed(2)}%
                </div>
                <div className="text-sm opacity-75">
                  Updated live
                </div>
              </div>

              <div className="bg-gradient-to-br from-orange-500 to-orange-600 rounded-xl p-6 text-white">
                <div className="text-sm opacity-90 mb-2">Active Deposits</div>
                <div className="text-2xl font-bold mb-1">
                  {balance.active_deposits}
                </div>
                <div className="text-sm opacity-75">
                  Earning interest
                </div>
              </div>
            </div>

            {/* Main Actions */}
            <div className="bg-white rounded-2xl shadow-xl p-6 mb-6">
              <div className="flex gap-2 mb-6 border-b">
                {['deposit', 'withdraw', 'lend'].map(tab => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={`px-6 py-3 font-medium transition-all ${
                      activeTab === tab
                        ? 'border-b-2 border-orange-500 text-orange-600'
                        : 'text-gray-600 hover:text-gray-900'
                    }`}
                  >
                    {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  </button>
                ))}
              </div>

              {activeTab === 'deposit' && (
                <div className="space-y-4">
                  <div className="bg-blue-50 p-4 rounded-xl flex items-start gap-3">
                    <Shield className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
                    <div className="text-sm text-blue-900">
                      Deposits are secured by Bitcoin Script with SPV verification. 
                      Earn algorithmic interest based on platform utilization.
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Deposit Amount (BSV)
                    </label>
                    <input
                      type="number"
                      step="0.00000001"
                      value={depositAmount}
                      onChange={(e) => setDepositAmount(e.target.value)}
                      className="w-full px-4 py-3 border border-gray-300 rounded-xl focus:ring-2 focus:ring-orange-500 focus:border-transparent"
                      placeholder="0.01"
                    />
                    {depositAmount && (
                      <div className="text-sm text-gray-600 mt-1">
                        ≈ ${formatUSD(parseFloat(depositAmount) * 100000000)} USD
                      </div>
                    )}
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Lock Period (Optional)
                    </label>
                    <div className="flex gap-2">
                      {[0, 7, 30, 90, 365].map(days => (
                        <button
                          key={days}
                          onClick={() => setLockDays(days)}
                          className={`flex-1 px-4 py-2 rounded-lg font-medium transition-all ${
                            lockDays === days
                              ? 'bg-orange-500 text-white'
                              : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                          }`}
                        >
                          {days === 0 ? 'None' : `${days}d`}
                        </button>
                      ))}
                    </div>
                    {lockDays > 0 && (
                      <div className="flex items-center gap-2 mt-2 text-sm text-green-600">
                        <TrendingUp className="w-4 h-4" />
                        Earn +{(lockDays / 365 * 2).toFixed(1)}% bonus APY for locking
                      </div>
                    )}
                  </div>

                  <button
                    onClick={handleDeposit}
                    disabled={loading || !depositAmount}
                    className="w-full bg-orange-500 hover:bg-orange-600 text-white py-4 rounded-xl font-bold transition-all shadow-lg disabled:opacity-50 flex items-center justify-center gap-2"
                  >
                    <ArrowDownToLine className="w-5 h-5" />
                    {loading ? 'Processing...' : 'Create Deposit'}
                  </button>
                </div>
              )}

              {activeTab === 'withdraw' && (
                <div className="space-y-4">
                  <div className="bg-yellow-50 p-4 rounded-xl flex items-start gap-3">
                    <Clock className="w-5 h-5 text-yellow-600 flex-shrink-0 mt-0.5" />
                    <div className="text-sm text-yellow-900">
                      Withdrawals are instant for unlocked deposits. 
                      Locked deposits must wait until the lock period expires.
                    </div>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Withdrawal Address
                    </label>
                    <input
                      type="text"
                      value={withdrawalAddress}
                      onChange={(e) => setWithdrawalAddress(e.target.value)}
                      className="w-full px-4 py-3 border border-gray-300 rounded-xl focus:ring-2 focus:ring-orange-500 focus:border-transparent font-mono text-sm"
                      placeholder="1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
                    />
                  </div>

                  <div className="border border-gray-200 rounded-xl p-4">
                    <h3 className="font-bold text-gray-900 mb-3">Available Deposits</h3>
                    {balance.active_deposits > 0 ? (
                      <div className="space-y-2">
                        {[...Array(balance.active_deposits)].map((_, idx) => (
                          <div key={idx} className="flex justify-between items-center bg-gray-50 p-3 rounded-lg">
                            <div>
                              <div className="font-mono text-sm">DEP_mock_{idx + 1}</div>
                              <div className="text-sm text-gray-600">
                                {formatBSV(500000)} BSV · Unlocked
                              </div>
                            </div>
                            <button
                              onClick={() => handleWithdraw(`DEP_mock_${idx + 1}`)}
                              disabled={loading}
                              className="bg-red-500 hover:bg-red-600 text-white px-4 py-2 rounded-lg font-medium transition-all disabled:opacity-50"
                            >
                              Withdraw
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="text-center text-gray-500 py-4">
                        No deposits available for withdrawal
                      </div>
                    )}
                  </div>
                </div>
              )}

              {activeTab === 'lend' && (
                <div className="space-y-4">
                  <div className="bg-purple-50 p-4 rounded-xl flex items-start gap-3">
                    <AlertCircle className="w-5 h-5 text-purple-600 flex-shrink-0 mt-0.5" />
                    <div className="text-sm text-purple-900">
                      P2P lending is enforced by Bitcoin Script. 
                      Loans are over-collateralized and automatically liquidated if needed.
                    </div>
                  </div>

                  <div className="text-center py-8">
                    <Send className="w-16 h-16 text-gray-400 mx-auto mb-4" />
                    <h3 className="text-xl font-bold text-gray-900 mb-2">Coming Soon</h3>
                    <p className="text-gray-600">
                      P2P lending marketplace launching in Phase 3
                    </p>
                  </div>
                </div>
              )}
            </div>

            {/* Transaction History */}
            {transactions.length > 0 && (
              <div className="bg-white rounded-2xl shadow-xl p-6">
                <h2 className="text-xl font-bold text-gray-900 mb-4">Recent Transactions</h2>
                <div className="space-y-2">
                  {transactions.map((tx, idx) => (
                    <div key={idx} className="flex justify-between items-center bg-gray-50 p-4 rounded-xl">
                      <div>
                        <div className="font-medium text-gray-900 capitalize">{tx.type}</div>
                        <div className="text-sm text-gray-600">
                          {tx.timestamp.toLocaleString()}
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="font-bold text-gray-900">
                          {formatBSV(tx.amount)} BSV
                        </div>
                        <div className={`text-sm ${
                          tx.status === 'Confirmed' ? 'text-green-600' : 'text-yellow-600'
                        }`}>
                          {tx.status}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}

        {!connected && (
          <div className="bg-white rounded-2xl shadow-xl p-12 text-center">
            <Wallet className="w-20 h-20 text-gray-400 mx-auto mb-6" />
            <h2 className="text-2xl font-bold text-gray-900 mb-4">
              Connect Your Wallet to Get Started
            </h2>
            <p className="text-gray-600 mb-6 max-w-md mx-auto">
              Connect with HandCash, Money Button, or any Paymail-compatible wallet 
              to start earning interest on your BSV deposits.
            </p>
            <button
              onClick={connectWallet}
              className="bg-orange-500 hover:bg-orange-600 text-white px-8 py-4 rounded-xl font-bold transition-all shadow-lg"
            >
              Connect Wallet
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default BSVBankApp;