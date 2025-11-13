import React, { useState, useEffect } from 'react';
import { ArrowRight, Plus, X, TrendingUp, Zap, Clock, CheckCircle } from 'lucide-react';

const PaymentChannels = ({ userPaymail = 'demo@test.io' }) => {
  const [channels, setChannels] = useState([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState('');
  const [activeView, setActiveView] = useState('channels');
  const [selectedChannel, setSelectedChannel] = useState(null);
  
  // Create channel form state
  const [newChannel, setNewChannel] = useState({
    counterparty: '',
    myBalance: '',
    theirBalance: '0'
  });
  
  // Payment form state
  const [payment, setPayment] = useState({
    amount: '',
    memo: ''
  });

  const API_BASE = 'http://localhost:8083';

  useEffect(() => {
    fetchChannels();
  }, []);

  const fetchChannels = async () => {
    setLoading(true);
    try {
      const response = await fetch(`${API_BASE}/channels/user/${userPaymail}`);
      const data = await response.json();
      setChannels(data.channels || []);
    } catch (error) {
      console.error('Failed to fetch channels:', error);
      setMessage('Failed to load channels');
    } finally {
      setLoading(false);
    }
  };

  const createChannel = async (e) => {
    e.preventDefault();
    setLoading(true);
    setMessage('');
    
    try {
      const response = await fetch(`${API_BASE}/channels/open`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          party_a_paymail: userPaymail,
          party_b_paymail: newChannel.counterparty,
          initial_balance_a: parseInt(newChannel.myBalance),
          initial_balance_b: parseInt(newChannel.theirBalance),
          timeout_blocks: 144
        })
      });
      
      const data = await response.json();
      
      if (response.ok) {
        setMessage(`✅ Channel created! ID: ${data.channel_id.substring(0, 16)}...`);
        setNewChannel({ counterparty: '', myBalance: '', theirBalance: '0' });
        fetchChannels();
        setActiveView('channels');
      } else {
        setMessage(`❌ ${data.message || 'Failed to create channel'}`);
      }
    } catch (error) {
      setMessage('❌ Network error');
    } finally {
      setLoading(false);
    }
  };

  const sendPayment = async (e) => {
    e.preventDefault();
    if (!selectedChannel) return;
    
    setLoading(true);
    setMessage('');
    
    const counterparty = selectedChannel.party_a_paymail === userPaymail
      ? selectedChannel.party_b_paymail
      : selectedChannel.party_a_paymail;
    
    try {
      const response = await fetch(`${API_BASE}/channels/${selectedChannel.channel_id}/payment`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          from_paymail: userPaymail,
          to_paymail: counterparty,
          amount_satoshis: parseInt(payment.amount),
          memo: payment.memo
        })
      });
      
      const data = await response.json();
      
      if (response.ok) {
        setMessage(`✅ Payment sent! ${payment.amount} sats`);
        setPayment({ amount: '', memo: '' });
        fetchChannels();
        // Refresh selected channel
        const updatedChannel = await fetch(`${API_BASE}/channels/${selectedChannel.channel_id}`);
        const channelData = await updatedChannel.json();
        setSelectedChannel(channelData);
      } else {
        setMessage(`❌ ${data.message || 'Payment failed'}`);
      }
    } catch (error) {
      setMessage('❌ Network error');
    } finally {
      setLoading(false);
    }
  };

  const getMyBalance = (channel) => {
    return channel.party_a_paymail === userPaymail
      ? channel.current_balance_a
      : channel.current_balance_b;
  };

  const getCounterparty = (channel) => {
    return channel.party_a_paymail === userPaymail
      ? channel.party_b_paymail
      : channel.party_a_paymail;
  };

  const formatSats = (amount) => {
    return amount.toLocaleString() + ' sats';
  };

  const getStatusColor = (status) => {
    const colors = {
      'Open': 'bg-yellow-500',
      'Active': 'bg-green-500',
      'Closed': 'bg-gray-500',
      'Disputed': 'bg-red-500'
    };
    return colors[status] || 'bg-gray-500';
  };

  if (loading && channels.length === 0) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-6 flex items-center justify-center">
        <div className="text-xl text-gray-600">Loading payment channels...</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-6">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <div className="flex items-center gap-3 mb-2">
            <Zap className="w-8 h-8 text-orange-500" />
            <h1 className="text-3xl font-bold text-gray-900">Payment Channels</h1>
          </div>
          <p className="text-gray-600">Instant, low-cost micropayments off-chain</p>
        </div>

        {/* Message Bar */}
        {message && (
          <div className={`mb-6 p-4 rounded-lg ${message.includes('❌') ? 'bg-red-50 text-red-800' : 'bg-green-50 text-green-800'}`}>
            {message}
          </div>
        )}

        {/* Navigation Tabs */}
        <div className="flex gap-4 mb-6 border-b border-gray-200">
          <button
            onClick={() => setActiveView('channels')}
            className={`px-6 py-3 font-semibold transition-colors ${
              activeView === 'channels'
                ? 'text-orange-600 border-b-2 border-orange-600'
                : 'text-gray-600 hover:text-orange-600'
            }`}
          >
            My Channels ({channels.length})
          </button>
          <button
            onClick={() => setActiveView('create')}
            className={`px-6 py-3 font-semibold transition-colors ${
              activeView === 'create'
                ? 'text-orange-600 border-b-2 border-orange-600'
                : 'text-gray-600 hover:text-orange-600'
            }`}
          >
            Create Channel
          </button>
        </div>

        {/* Channels List View */}
        {activeView === 'channels' && (
          <div className="space-y-4">
            {channels.length === 0 ? (
              <div className="bg-white rounded-xl shadow-sm p-12 text-center">
                <Zap className="w-16 h-16 text-gray-300 mx-auto mb-4" />
                <h3 className="text-xl font-semibold text-gray-900 mb-2">No Payment Channels</h3>
                <p className="text-gray-600 mb-6">Create your first channel to start sending instant payments</p>
                <button
                  onClick={() => setActiveView('create')}
                  className="bg-gradient-to-r from-orange-500 to-orange-600 text-white px-6 py-3 rounded-lg font-semibold hover:from-orange-600 hover:to-orange-700 transition-colors"
                >
                  Create Channel
                </button>
              </div>
            ) : (
              channels.map(channel => (
                <div
                  key={channel.id}
                  className="bg-white rounded-xl shadow-sm p-6 hover:shadow-md transition-shadow cursor-pointer"
                  onClick={() => setSelectedChannel(channel)}
                >
                  <div className="flex items-start justify-between mb-4">
                    <div>
                      <div className="flex items-center gap-2 mb-1">
                        <span className="font-semibold text-gray-900">
                          {getCounterparty(channel)}
                        </span>
                        <span className={`px-2 py-1 rounded-full text-xs text-white ${getStatusColor(channel.status)}`}>
                          {channel.status}
                        </span>
                      </div>
                      <p className="text-sm text-gray-500">
                        Channel ID: {channel.channel_id.substring(0, 16)}...
                      </p>
                    </div>
                    <div className="text-right">
                      <div className="text-2xl font-bold text-gray-900">
                        {formatSats(getMyBalance(channel))}
                      </div>
                      <div className="text-sm text-gray-500">Your Balance</div>
                    </div>
                  </div>
                  
                  <div className="grid grid-cols-3 gap-4 text-sm">
                    <div>
                      <div className="text-gray-500">Sequence</div>
                      <div className="font-semibold">#{channel.sequence_number}</div>
                    </div>
                    <div>
                      <div className="text-gray-500">Opened</div>
                      <div className="font-semibold">
                        {new Date(channel.opened_at).toLocaleDateString()}
                      </div>
                    </div>
                    <div>
                      <div className="text-gray-500">Timeout</div>
                      <div className="font-semibold">{channel.timeout_blocks} blocks</div>
                    </div>
                  </div>
                  
                  {channel.status === 'Active' || channel.status === 'Open' ? (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelectedChannel(channel);
                      }}
                      className="mt-4 w-full bg-gradient-to-r from-orange-500 to-orange-600 text-white py-2 rounded-lg font-semibold hover:from-orange-600 hover:to-orange-700 transition-colors flex items-center justify-center gap-2"
                    >
                      <Zap className="w-4 h-4" />
                      Send Payment
                    </button>
                  ) : null}
                </div>
              ))
            )}
          </div>
        )}

        {/* Create Channel View */}
        {activeView === 'create' && (
          <div className="bg-white rounded-xl shadow-sm p-8 max-w-2xl mx-auto">
            <h2 className="text-2xl font-bold text-gray-900 mb-6">Create Payment Channel</h2>
            
            <form onSubmit={createChannel} className="space-y-6">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Counterparty Paymail
                </label>
                <input
                  type="text"
                  value={newChannel.counterparty}
                  onChange={(e) => setNewChannel({ ...newChannel, counterparty: e.target.value })}
                  placeholder="user@handcash.io"
                  className="w-full px-4 py-3 border-2 border-gray-200 rounded-lg focus:border-orange-500 focus:outline-none"
                  required
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Your Initial Balance (satoshis)
                </label>
                <input
                  type="number"
                  value={newChannel.myBalance}
                  onChange={(e) => setNewChannel({ ...newChannel, myBalance: e.target.value })}
                  placeholder="100000"
                  className="w-full px-4 py-3 border-2 border-gray-200 rounded-lg focus:border-orange-500 focus:outline-none"
                  required
                  min="1"
                />
                <p className="text-sm text-gray-500 mt-1">
                  This amount will be locked in the channel
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Their Initial Balance (satoshis) - Optional
                </label>
                <input
                  type="number"
                  value={newChannel.theirBalance}
                  onChange={(e) => setNewChannel({ ...newChannel, theirBalance: e.target.value })}
                  placeholder="0"
                  className="w-full px-4 py-3 border-2 border-gray-200 rounded-lg focus:border-orange-500 focus:outline-none"
                  min="0"
                />
                <p className="text-sm text-gray-500 mt-1">
                  Usually 0 - they can fund later
                </p>
              </div>

              <div className="bg-orange-50 border border-orange-200 rounded-lg p-4">
                <p className="text-sm text-orange-800">
                  <strong>Note:</strong> This creates a payment channel for instant off-chain transactions. 
                  Both parties can send payments back and forth with sub-second confirmation and minimal fees.
                </p>
              </div>

              <button
                type="submit"
                disabled={loading}
                className="w-full bg-gradient-to-r from-orange-500 to-orange-600 text-white py-3 rounded-lg font-semibold hover:from-orange-600 hover:to-orange-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {loading ? 'Creating...' : 'Create Channel'}
              </button>
            </form>
          </div>
        )}

        {/* Payment Modal */}
        {selectedChannel && (
          <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
            <div className="bg-white rounded-xl shadow-xl max-w-md w-full p-6">
              <div className="flex items-center justify-between mb-6">
                <h3 className="text-xl font-bold text-gray-900">Send Payment</h3>
                <button
                  onClick={() => setSelectedChannel(null)}
                  className="text-gray-400 hover:text-gray-600"
                >
                  <X className="w-6 h-6" />
                </button>
              </div>

              <div className="mb-6 p-4 bg-gray-50 rounded-lg">
                <div className="text-sm text-gray-600 mb-1">To</div>
                <div className="font-semibold text-gray-900">{getCounterparty(selectedChannel)}</div>
                <div className="text-sm text-gray-600 mt-3">Your Available Balance</div>
                <div className="text-2xl font-bold text-orange-600">
                  {formatSats(getMyBalance(selectedChannel))}
                </div>
              </div>

              <form onSubmit={sendPayment} className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Amount (satoshis)
                  </label>
                  <input
                    type="number"
                    value={payment.amount}
                    onChange={(e) => setPayment({ ...payment, amount: e.target.value })}
                    placeholder="1000"
                    className="w-full px-4 py-3 border-2 border-gray-200 rounded-lg focus:border-orange-500 focus:outline-none"
                    required
                    min="1"
                    max={getMyBalance(selectedChannel)}
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Memo (optional)
                  </label>
                  <input
                    type="text"
                    value={payment.memo}
                    onChange={(e) => setPayment({ ...payment, memo: e.target.value })}
                    placeholder="Coffee payment"
                    className="w-full px-4 py-3 border-2 border-gray-200 rounded-lg focus:border-orange-500 focus:outline-none"
                  />
                </div>

                <div className="flex gap-3">
                  <button
                    type="button"
                    onClick={() => setSelectedChannel(null)}
                    className="flex-1 px-4 py-3 border-2 border-gray-200 text-gray-700 rounded-lg font-semibold hover:bg-gray-50 transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    disabled={loading}
                    className="flex-1 bg-gradient-to-r from-orange-500 to-orange-600 text-white py-3 rounded-lg font-semibold hover:from-orange-600 hover:to-orange-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                  >
                    <Zap className="w-4 h-4" />
                    {loading ? 'Sending...' : 'Send'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default PaymentChannels;