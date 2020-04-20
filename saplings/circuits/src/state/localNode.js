/**
 * Copyright 2018-2020 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import React, { useEffect, useState } from 'react';
import PropTypes from 'prop-types';
import { getNodeID } from '../api/splinter';

const LocalNodeContext = React.createContext();

function LocalNodeProvider({ children }) {
  const [nodeState, setNodeID] = useState({ isSet: false, nodeID: 'unknown' });
  useEffect(() => {
    const getNode = async () => {
      if (!nodeState.isSet) {
        try {
          const node = await getNodeID();
          setNodeID({ isSet: true, nodeID: node });
        } catch (e) {
          throw Error(`Error fetching node information: ${e}`);
        }
      }
    };
    getNode();
  });

  return (
    <LocalNodeContext.Provider value={nodeState.nodeID}>
      {children}
    </LocalNodeContext.Provider>
  );
}

LocalNodeProvider.propTypes = {
  children: PropTypes.node.isRequired
};

function useLocalNodeState() {
  const context = React.useContext(LocalNodeContext);
  if (context === undefined) {
    throw new Error(
      'useLocalNodeState must be used within a LocalNodeProvider'
    );
  }
  return context;
}

export { LocalNodeProvider, useLocalNodeState };
