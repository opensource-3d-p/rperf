#!/usr/bin/env python3
"""
Copyright (C) 2021 Evtech Solutions, Ltd., dba 3D-P
Copyright (C) 2021 Neil Tallim <neiltallim@3d-p.com>

This file is part of rperf.

rperf is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

rperf is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with rperf.  If not, see <https://www.gnu.org/licenses/>.
"""
"""
This is a simple testing script to ensure basic correctness of rperf behaviour.

It doesn't test off-host connectivity (unless you've got a very unusual setup)
and it isn't concerned with probing limits. It just makes sure TCP, UDP, and
common options are functioning as expected.
"""

import argparse
import concurrent.futures
import json
import subprocess
import unittest

_RPERF_BINARY = "./target/release/rperf"
_DISPLAY_LOGS = False #whether to display rperf's logs on stderr while testing

def _run_rperf_client(address, args):
    full_args = [_RPERF_BINARY, '-c', address, '-f', 'json',]
    full_args.extend(args)
    if _DISPLAY_LOGS:
        result = subprocess.check_output(full_args)
    else:
        result = subprocess.check_output(full_args, stderr=subprocess.DEVNULL)
    return json.loads(result)
    
def _run_rperf_client_hostname(args):
    return _run_rperf_client('localhost', args)
    
def _run_rperf_client_ipv4(args):
    return _run_rperf_client('127.0.0.1', args)
    
def _run_rperf_client_ipv6(args):
    return _run_rperf_client('::1', args)




class TestIpv4(unittest.TestCase):
    def setUp(self):
        if _DISPLAY_LOGS:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s',))
        else:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s',), stderr=subprocess.DEVNULL)
            
    def tearDown(self):
        self.server.terminate()
        self.server.wait()
        
    def test_tcp_forward(self):
        result = _run_rperf_client_ipv4((
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-P', '2', #two parallel streams
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 2)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['config']['additional']['ip_version'], 4)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 2000000, delta=50000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 4.0, delta=0.1)
        
    def test_tcp_reverse(self):
        result = _run_rperf_client_ipv4((
            '-R', #run in reverse mode
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-P', '2', #two parallel streams
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 2)
        self.assertEqual(result['config']['additional']['reverse'], True)
        self.assertEqual(result['config']['additional']['ip_version'], 4)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 2000000, delta=50000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 4.0, delta=0.1)
        
    def test_udp_forward(self):
        result = _run_rperf_client_ipv4((
            '-u', #run UDP test
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '1200', #try to send 1200 bytes of data at a time
            '-P', '2', #two parallel streams
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'udp')
        self.assertEqual(result['config']['common']['streams'], 2)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['config']['additional']['ip_version'], 4)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertEqual(result['summary']['packets_received'], result['summary']['packets_sent'])
        self.assertEqual(result['summary']['framed_packet_size'], 1208)
        self.assertEqual(result['summary']['packets_duplicated'], 0)
        self.assertEqual(result['summary']['packets_lost'], 0)
        self.assertEqual(result['summary']['packets_out_of_order'], 0)
        self.assertAlmostEqual(result['summary']['bytes_received'], 2000000, delta=50000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 4.0, delta=0.1)
        
    def test_udp_reverse(self):
        result = _run_rperf_client_ipv4((
            '-u', #run UDP test
            '-R', #run in reverse mode
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '1200', #try to send 1200 bytes of data at a time
            '-P', '2', #two parallel streams
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'udp')
        self.assertEqual(result['config']['common']['streams'], 2)
        self.assertEqual(result['config']['additional']['reverse'], True)
        self.assertEqual(result['config']['additional']['ip_version'], 4)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertEqual(result['summary']['packets_received'], result['summary']['packets_sent'])
        self.assertEqual(result['summary']['framed_packet_size'], 1208)
        self.assertEqual(result['summary']['packets_duplicated'], 0)
        self.assertEqual(result['summary']['packets_lost'], 0)
        self.assertEqual(result['summary']['packets_out_of_order'], 0)
        self.assertAlmostEqual(result['summary']['bytes_received'], 2000000, delta=50000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 4.0, delta=0.1)




class TestIpv6(unittest.TestCase):
    def setUp(self):
        if _DISPLAY_LOGS:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s', '-6',))
        else:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s', '-6'), stderr=subprocess.DEVNULL)
            
    def tearDown(self):
        self.server.terminate()
        self.server.wait()
        
    def test_tcp_forward(self):
        result = _run_rperf_client_ipv6((
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['config']['additional']['ip_version'], 6)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 1000000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 2.0, delta=0.1)
        
    def test_tcp_reverse(self):
        result = _run_rperf_client_ipv6((
            '-R', #run in reverse mode
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], True)
        self.assertEqual(result['config']['additional']['ip_version'], 6)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 1000000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 2.0, delta=0.1)
        
    def test_udp_forward(self):
        result = _run_rperf_client_ipv6((
            '-u', #run UDP test
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '1200', #try to send 1200 bytes of data at a time
            '-t', '1', #run for one second
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'udp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['config']['additional']['ip_version'], 6)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertEqual(result['summary']['packets_received'], result['summary']['packets_sent'])
        self.assertEqual(result['summary']['framed_packet_size'], 1208)
        self.assertEqual(result['summary']['packets_duplicated'], 0)
        self.assertEqual(result['summary']['packets_lost'], 0)
        self.assertEqual(result['summary']['packets_out_of_order'], 0)
        self.assertAlmostEqual(result['summary']['bytes_received'], 500000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 1.0, delta=0.1)
        
    def test_udp_reverse(self):
        result = _run_rperf_client_ipv6((
            '-u', #run UDP test
            '-R', #run in reverse mode
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '1200', #try to send 1200 bytes of data at a time
            '-t', '1', #run for one seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'udp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], True)
        self.assertEqual(result['config']['additional']['ip_version'], 6)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertEqual(result['summary']['packets_received'], result['summary']['packets_sent'])
        self.assertEqual(result['summary']['framed_packet_size'], 1208)
        self.assertEqual(result['summary']['packets_duplicated'], 0)
        self.assertEqual(result['summary']['packets_lost'], 0)
        self.assertEqual(result['summary']['packets_out_of_order'], 0)
        self.assertAlmostEqual(result['summary']['bytes_received'], 500000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 1.0, delta=0.1)
        



class TestMisc(unittest.TestCase):
    def setUp(self):
        if _DISPLAY_LOGS:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s', '-6', '-A', '0,1'))
        else:
            self.server = subprocess.Popen((_RPERF_BINARY, '-s', '-6', '-A', '0,1'), stderr=subprocess.DEVNULL)
            
    def tearDown(self):
        self.server.terminate()
        self.server.wait()
        
    def test_hostname(self):
        result = _run_rperf_client_hostname((
            '-N', #disable Nagle's algorithm
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 1000000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 1.75, delta=0.5)
        
    def test_hostname_reverse(self):
        result = _run_rperf_client_hostname((
            '-N', #disable Nagle's algorithm
            '-R', #run in reverse mode
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 1)
        self.assertEqual(result['config']['additional']['reverse'], True)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 1000000, delta=25000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 1.75, delta=0.5)
        
    def test_ipv4_mapped_with_core_affinity(self):
        result = _run_rperf_client_ipv4((
            '-A', '2,3', #set CPU core-affinity to 2 and 3
            '-b', '500000', #keep it light, at 500kBps per stream
            '-l', '4096', #try to send 4k of data at a time
            '-P', '2', #two parallel streams
            '-t', '2', #run for two seconds
        ))
        self.assertTrue(result['success'])
        self.assertEqual(result['config']['common']['family'], 'tcp')
        self.assertEqual(result['config']['common']['streams'], 2)
        self.assertEqual(result['config']['additional']['reverse'], False)
        self.assertEqual(result['config']['additional']['ip_version'], 4)
        self.assertEqual(result['summary']['bytes_received'], result['summary']['bytes_sent'])
        self.assertAlmostEqual(result['summary']['bytes_received'], 2000000, delta=50000)
        self.assertAlmostEqual(result['summary']['duration_receive'], 3.5, delta=1.0)
        
    def test_multiple_simultaneous_clients(self):
        with concurrent.futures.ThreadPoolExecutor() as executor:
            tcp_1_ipv4 = executor.submit(_run_rperf_client_ipv4, (
                '-b', '100000', #keep it light, at 100kBps per stream
                '-l', '4096', #try to send 4k of data at a time
                '-t', '2', #run for two seconds
            ))
            tcp_2_ipv6_reverse = executor.submit(_run_rperf_client_ipv6, (
                '-R', #run in reverse mode
                '-b', '100000', #keep it light, at 100kBps per stream
                '-l', '4096', #try to send 4k of data at a time
                '-t', '2', #run for two seconds
            ))
            udp_1_ipv6 = executor.submit(_run_rperf_client_ipv6, (
                '-u', #run UDP test
                '-b', '100000', #keep it light, at 100kBps per stream
                '-l', '1200', #try to send 1200 bytes of data at a time
                '-t', '2', #run for two seconds
            ))
            udp_2_hostname_reverse = executor.submit(_run_rperf_client_hostname, (
                '-u', #run UDP test
                '-R', #run in reverse mode
                '-b', '100000', #keep it light, at 100kBps per stream
                '-l', '1200', #try to send 1200 bytes of data at a time
                '-t', '2', #run for two seconds
            ))
            
            self.assertTrue(tcp_1_ipv4.result()['success'])
            self.assertTrue(tcp_2_ipv6_reverse.result()['success'])
            self.assertTrue(udp_1_ipv6.result()['success'])
            self.assertTrue(udp_2_hostname_reverse.result()['success'])




if __name__ == '__main__':
    unittest.main()
    
